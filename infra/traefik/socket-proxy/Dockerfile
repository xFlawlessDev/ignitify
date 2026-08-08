FROM tecnativa/docker-socket-proxy:0.1.2@sha256:dc8ec925b1360c54e6bf350602d6faac4e33c5d8d809118e4c000c0b14a4529a

COPY entrypoint.sh /usr/local/bin/ignitify-socket-proxy-entrypoint
RUN chmod 0555 /usr/local/bin/ignitify-socket-proxy-entrypoint

ENTRYPOINT ["/usr/local/bin/ignitify-socket-proxy-entrypoint"]
CMD ["haproxy", "-W", "-db", "-f", "/tmp/haproxy.cfg"]
