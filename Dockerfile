FROM rust:1-bullseye AS botman-builder

WORKDIR /app
COPY . .
RUN cargo build -r

FROM rust:1-bullseye

RUN apt update && apt install -y git make curl tar
RUN mkdir /opt/nvim
RUN curl -fsSL https://github.com/neovim/neovim/releases/download/v0.7.2/nvim-linux64.tar.gz -o /opt/nvim.tar.gz
RUN tar -xvzf /opt/nvim.tar.gz --strip-components=1 -C /opt/nvim
ENV PATH="/opt/nvim/bin:${PATH}"

RUN mkdir -p ~/.local/share/nvim/site/vendor/start
RUN git clone --depth 1 https://github.com/williamboman/mason.nvim ~/.local/share/nvim/site/vendor/start/
ENV PATH="~/.local/share/nvim/mason/bin:${PATH}"

RUN nvim --headless -c "MasonInstall stylua" -c "qall"
RUN command -v stylua

RUN git config --global user.name "williambotman[bot]" && \
    git config --global user.email "william+bot@redwill.se"

WORKDIR /app
COPY --from=botman-builder /app/target/release/botman /usr/local/bin/botman
ENV ROCKET_ENV=production

EXPOSE 80

CMD [ "botman" ]
