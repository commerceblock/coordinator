FROM alpine:latest

ENV PATH=$PATH:/usr/src/jsonrpc-cli
WORKDIR /usr/src

#    && curl -LO https://raw.githubusercontent.com/commerceblock/coordinator/develop/scripts/create_request.sh \

COPY scripts/create_request.sh /usr/src

RUN set -x \
    && apk --no-cache add bash jq bc curl php composer git \
    && chmod +x create_request.sh \
    && git clone https://github.com/dan-da/jsonrpc-cli \
    && composer install -d jsonrpc-cli \
    && echo "*/15    *       *       *       *       bash /usr/src/create_request.sh \$(echo \${CLIENT_GENESIS_HASH}) 100 10 5 5 25 50 \$(echo \${PRIV_KEY_ADDR})" \
       >> /var/spool/cron/crontabs/root

CMD ["bash", "-c"]
