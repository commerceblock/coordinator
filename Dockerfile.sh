FROM alpine:latest

ENV PATH=$PATH:/usr/src/jsonrpc-cli
COPY scripts /usr/src/scripts
WORKDIR /usr/src

RUN set -x \
    && apk --no-cache add bash jq bc curl php composer git \
    && chmod +x scripts/*.sh \
    && git clone https://github.com/dan-da/jsonrpc-cli \
    && composer install -d jsonrpc-cli \
    && echo "*/15    *       *       *       *       bash /usr/src/scripts/create_request.sh \
         \$(echo \${CLIENT_GENESIS_HASH}) \
         \$(echo \${START_PRICE}) \
         \$(echo \${END_PRICE}) \
         \$(echo \${AUCTION_DURATION}) \
         \$(echo \${REQUEST_DURATION}) \
         \$(echo \${NUM_TICKETS}) \
         \$(echo \${FEE_PERCENTAGE}) \
         \$(echo \${PRIV_KEY_ADDR}) \
         \$(echo \${TXID}) \
         \$(echo \${VOUT})" \
        > /var/spool/cron/crontabs/root \
    && sed -i 's/          / /g' /var/spool/cron/crontabs/root

CMD ["bash", "-c"]
