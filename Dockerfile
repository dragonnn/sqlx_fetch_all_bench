FROM mcr.microsoft.com/vscode/devcontainers/rust:1-bullseye

RUN apt update && apt install -y gnuplot