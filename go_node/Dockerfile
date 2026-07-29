FROM golang:1.22-alpine AS builder
WORKDIR /app
COPY main.go .
RUN go mod init go_node && CGO_ENABLED=0 GOOS=linux go build -o cell_node main.go

FROM alpine:latest
WORKDIR /root/
COPY --from=builder /app/cell_node .
CMD ["./cell_node"]
