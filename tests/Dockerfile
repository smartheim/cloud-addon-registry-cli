FROM alpine:latest as builder
RUN apk add --no-cache make cmake g++
RUN mkdir cpp && mkdir cpp/build
WORKDIR /cpp
COPY main.cpp CMakeLists.txt .
WORKDIR /cpp/build
RUN rm -rf * && cmake -DCMAKE_BUILD_TYPE=Release ../ && cmake --build -j4 .
RUN strip app
RUN ls -lh | grep app

FROM scratch
COPY --from=builder /cpp/build/app /app
ENTRYPOINT ["/app"]
