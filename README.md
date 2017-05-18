## esper

Esper is a standalone Event Source /
[Server Sent Events](https://developer.mozilla.org/en-US/docs/Web/API/Server-sent_events/Using_server-sent_events) (SSE) broker. It is powered by the most excellent event
driven [hyper](https://github.com/hyperium/hyper) library.

### Usage

There are two main routes provided by esper; one for subscribing and one
for publishing:

- `GET /subscribe/:topic_id`
  - When requested with a valid `topic_id`, this route will respond with
    the Event Source content type and will leave the connection open.
The client is now subscribed for the given `topic_id` and will receive
all published messages for this topic.

- `POST /publish/:topic_id`
  - When requested with a valid `topic_id`, this route will publish a
    message to all subscribed clients. The entire POST body is
considered to be the message payload. Thus, the POST data should be
formatted like a server-sent events and include a `data` field with an
optional `event` and `id` field.

The `:topic_id` is specified as the second part of the request path.
This ID must be alphanumeric characters only (case insensitive) and must
be between 8 and 64 characters in length. Further validation may be
introduced at a later time.

### Authentication

Esper uses JSON Web Tokens to ensure requests are legitimate. For tokens
to be considered valid, they must include an `exp` field set to some
future timestamp (in seconds as an integer) as well as a `sub` field set
to the `topic_id`.

Authentication is available for both the subscribe and publish routes
and is enabled by setting one or both environmental variables. These
variables are named `ESPER_SUBSCRIBER_SECRET` and `ESPER_PUBLISHER_SECRET`.
It is possible to enable just one kind of authentication by leaving the
other secret undefined.

Also, please note that, the `/stats` route is protected by JWT using the
publisher secret since this route is intended for developer use.

### Examples

Here is a quick example of what to expect from esper using `curl`.
First, we subscribe a client to the topic `abcdef123`:

```bash
curl -v -X GET http://localhost:3000/subscribe/abcdef123
> GET /subscribe/abcdef123 HTTP/1.1
> Host: localhost:3001
>
< HTTP/1.1 200 OK
< Content-Type: text/event-stream
< Date: Mon, 11 Jul 2016 20:27:53 GMT
< Transfer-Encoding: chunked
<
```

Next, using another shell, lets publish a message with `curl` to the
same topic:

```bash
curl -v -X POST http://localhost:3000/publish/abcdef123 -d $'event:
testing\ndata: {some:"data"}'
> POST /publish/abcdef123 HTTP/1.1
> Host: localhost:3001
> Content-Length: 36
> Content-Type: application/x-www-form-urlencoded
```

Now if we go back to the first subscribed `curl` client, we will see the
following output:

```bash
event: testing
data: {"some":"data"}
```

We can also subscribe using the `EventSource` object in the browser.
In fact, esper is mainly designed for this use case!

A quick example use some JavaScript would be as follows:

```javascript
var evtSource = new EventSource('localhost:3000/subscribe/abcdef123');

evtSource.onmessage = function(evt) {
    console.log(evt.data);
};
```

There you have it, the essence of esper!


### Building

Esper can be build with stable Rust and `cargo` command-line tool
```bash
git clone https://github.com/mikeycgto/esper.git && cd esper
cargo build --release 
```

### Deploying

Esper ships as a standalone executable with a small set of command-line
options. Here is esper's help screen for more details on the supported
options:

```
$ esper --help
esper - Event Source HTTP server, powered by hyper.

Usage:
  esper [--bind=<bind>] [--port=<port>] [--threads=<st>]
  esper (-h | --help)
  esper --version

Options:
  -h --help          Show this screen.
  --version          Show version.
  -b --bind=<bind>   Bind to specific IP [default: 127.0.0.1]
  -p --port=<port>   Run on a specific port number [default: 3000]
  -t --threads=<st>  Number of server threads [default: 2].
  --no-auth          Run without JWT authentication.
```

### Docker

Esper can run in docker container

#### Create docker image and build
```
git clone https://github.com/mikeycgto/esper.git && cd esper
docker build -t unique-image-name .
```

#### Run in interactive mode (twice CTRL+C for exit)
```
docker run -it -p 3000:3000 unique-image-name
```

#### Run in background (Daemon)

```
# First launch
docker run -d --name esper_server_name -p 3000:3000

# Stop
docker stop esper_server_name

# Start
docker start esper_server_name
```

