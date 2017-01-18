FROM scorpil/rust:1.16
MAINTAINER david david@shippeo.com

RUN apt-get -qq update
RUN apt-get -qq -y --no-install-recommends install \
	libssl-dev \
	# ring dependencies : https://github.com/briansmith/ring/blob/master/BUILDING.md#building-the-rust-library
	build-essential

ADD . /src
WORKDIR /src

EXPOSE 3000

RUN cargo build --release
CMD cargo run --release -- --bind 0.0.0.0
