use criterion::{criterion_group, criterion_main, Criterion, PlottingBackend};
use sqlx::{Executor, PgPool};
use std::env;
use uuid::Uuid;

#[derive(PartialEq, Clone, pg_mapper::TryFromRow, sqlx::FromRow)]
pub struct Data {
    pub datetime: chrono::NaiveDateTime,
    pub measuring_result: f64,
    pub alert_alarm: bool,
    pub danger_alarm: bool,
    pub work_state_id: Option<i32>,
    pub device_uuid: Uuid,
}

pub fn sqlx_fetch_all_bench(c: &mut Criterion) {
    // begin setup

    // create the tokio runtime to be used for the benchmarks
    let rt = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .unwrap();

    let device_uuid = Uuid::new_v4();
    let start_date = chrono::Local::now().naive_local() - chrono::Duration::days(29);
    let end_date = chrono::Local::now().naive_local();

    // seed the data server side, get a handle to the collection
    let (sqlx_pool, tokio_postgres) = rt.block_on(async {
        let sqlx_pool = PgPool::connect(&env::var("DATABASE_URL").unwrap())
            .await
            .unwrap();

        sqlx_pool
            .execute("DROP TABLE IF EXISTS data")
            .await
            .unwrap();
        sqlx_pool
            .execute(
                "CREATE TABLE data (
                datetime timestamp NOT NULL,
                measuring_result double precision NOT NULL,
                alert_alarm bool NOT NULL,
                danger_alarm bool NOT NULL,
                device_uuid uuid NOT NULL,
                work_state_id integer,
                CONSTRAINT data_pk PRIMARY KEY (datetime,device_uuid)
            
            )",
            )
            .await
            .unwrap();
        sqlx_pool
            .execute(
                "SELECT create_hypertable('data','datetime', chunk_time_interval => INTERVAL '1d')",
            )
            .await
            .unwrap();
        sqlx_pool
            .execute(
                format!(
                    "INSERT INTO data 
                        SELECT x AS datetime,
                                random()*100 AS measuring_result, 
                                false AS alert_alarm, 
                                false AS danger_alarm,
                                '{}'::uuid AS device_uuid,
                                NULL AS work_state_id
                                FROM generate_series('{}','{}', INTERVAL '0.1s') AS x;",
                    device_uuid,
                    start_date.date(),
                    end_date.date()
                )
                .as_str(),
            )
            .await
            .unwrap();

        use tokio_postgres::NoTls;

        let (client, connection) =
            tokio_postgres::connect(&env::var("DATABASE_URL").unwrap(), NoTls)
                .await
                .unwrap();
        tokio::spawn(async move {
            if let Err(e) = connection.await {
                eprintln!("connection error: {}", e);
            }
        });

        (sqlx_pool, client)
    });
    // end setup
    c.bench_function("sqlx_query_method", |b| {
        b.to_async(&rt).iter( || {
            // begin measured portion of benchmark
            async {
                let data = sqlx::query("SELECT * FROM data WHERE device_uuid = ($1) AND (datetime >= $2 AND ($3 OR datetime <= $4)) ORDER BY datetime ASC").bind(device_uuid
                        ).bind(start_date).bind(false).bind(end_date).fetch_all(&sqlx_pool).await.unwrap();
                assert_ne!(data.len(), 0);
            }
        })
    });

    // end setup
    c.bench_function("sqlx_query_as_method", |b| {
        b.to_async(&rt).iter( || {
            // begin measured portion of benchmark
            async {
                let data: Vec<Data> = sqlx::query_as("SELECT * FROM data WHERE device_uuid = ($1) AND (datetime >= $2 AND ($3 OR datetime <= $4)) ORDER BY datetime ASC").bind(device_uuid
                        ).bind(start_date).bind(false).bind(end_date).fetch_all(&sqlx_pool).await.unwrap();
                assert_ne!(data.len(), 0);
            }
        })
    });

    c.bench_function("sqlx_query_as", |b| {

        b.to_async(&rt).iter(|| {
            // begin measured portion of benchmark
            async {
                let data = sqlx::query_as!(Data, "SELECT * FROM data WHERE device_uuid = ($1) AND (datetime >= $2 AND ($3 OR datetime <= $4)) ORDER BY datetime ASC",
                        device_uuid,
                        start_date,
                        false,
                        end_date).fetch_all(&sqlx_pool).await.unwrap();
                assert_ne!(data.len(), 0);
            }
        })
    });

    c.bench_function("sqlx_query", |b| {
        b.to_async(&rt).iter( || {
            // begin measured portion of benchmark
            async {
                let data = sqlx::query!("SELECT * FROM data WHERE device_uuid = ($1) AND (datetime >= $2 AND ($3 OR datetime <= $4)) ORDER BY datetime ASC",
                        device_uuid,
                        start_date,
                        false,
                        end_date).fetch_all(&sqlx_pool).await.unwrap();
                assert_ne!(data.len(), 0);
            }
        })
    });

    c.bench_function("tokio_postgres", |b| {
        b.to_async(&rt).iter( || {
            // begin measured portion of benchmark
            async {
                let data = tokio_postgres.query("SELECT * FROM data WHERE device_uuid = ($1) AND (datetime >= $2 AND ($3 OR datetime <= $4)) ORDER BY datetime ASC",
                        &[ &device_uuid,
                        &start_date,
                        &false,
                        &end_date]).await.unwrap();
                assert_ne!(data.len(), 0);
            }
        })
    });

    c.bench_function("tokio_postgres_map", |b| {
        b.to_async(&rt).iter( || {
            // begin measured portion of benchmark
            async {
                let data: Vec<Data> = tokio_postgres.query("SELECT * FROM data WHERE device_uuid = ($1) AND (datetime >= $2 AND ($3 OR datetime <= $4)) ORDER BY datetime ASC",
                        &[ &device_uuid,
                        &start_date,
                        &false,
                        &end_date]).await.unwrap().into_iter().map(|r| r.try_into().unwrap()).collect();
                assert_ne!(data.len(), 0);
            }
        })
    });
}

use std::time::Duration;

criterion_group! {
    name = benches;
    config = Criterion::default().measurement_time(Duration::from_secs(500)).warm_up_time(Duration::from_secs(50)).sample_size(10).nresamples(10).plotting_backend(PlottingBackend::Gnuplot);
    targets = sqlx_fetch_all_bench
}
criterion_main!(benches);
