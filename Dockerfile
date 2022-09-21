FROM rust:1-bullseye AS botman-builder

WORKDIR /app
COPY . .
RUN rustup toolchain install nightly-aarch64-unknown-linux-gnu
RUN cargo +nightly build -r

FROM debian:bullseye

# Install & setup Neovim
RUN apt update && apt install -y git make curl tar unzip
RUN mkdir /opt/nvim
RUN curl -fsSL https://github.com/neovim/neovim/releases/download/v0.7.2/nvim-linux64.tar.gz -o /opt/nvim.tar.gz
RUN tar -xvzf /opt/nvim.tar.gz --strip-components=1 -C /opt/nvim
ENV PATH="/opt/nvim/bin:${PATH}"

# Install runtime deps
RUN curl -fsSL -o /tmp/stylua.zip https://github.com/JohnnyMorganz/StyLua/releases/download/v0.14.3/stylua-linux.zip && \
    unzip /tmp/stylua.zip -d /usr/local/bin && \
    rm -f /tmp/stylua.zip

# Configure git
RUN git config --global user.name "williambotman[bot]" && \
    git config --global user.email "william+bot@redwill.se"

WORKDIR /app
COPY --from=botman-builder /app/target/release/botman /usr/local/bin/botman
ENV ROCKET_ENV=production

EXPOSE 80

CMD [ "botman" ]
