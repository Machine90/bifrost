# use a plain alpine image
FROM frolvlad/alpine-glibc:alpine-3
# if needed, install additional dependencies here
# RUN sed -i 's#https\?://dl-cdn.alpinelinux.org/alpine#https://mirrors.tuna.tsinghua.edu.cn/alpine#g' /etc/apk/repositories
RUN sed -i 's/dl-cdn.alpinelinux.org/mirrors.aliyun.com/g' /etc/apk/repositories

RUN apk update
RUN apk add libgcc libpq cmake

# setup application
WORKDIR /home/gateway
RUN mkdir -p /home/gateway
RUN mkdir -p /home/gateway/build/local/filters/

COPY ./bifrost /home/gateway/bifrost
COPY ./config.toml /home/gateway/config.toml
COPY ./filters/* /home/gateway/build/local/filters/

EXPOSE 8000

# set the binary as entrypoint
CMD ["./bifrost"]
