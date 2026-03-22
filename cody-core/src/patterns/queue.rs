use once_cell::sync::Lazy;
use regex::Regex;

// RabbitMQ: channel.publish(exchange, routingKey, ...)
pub static RABBITMQ_PUBLISH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"channel\.(?:publish|sendToQueue)\s*\(\s*['"`]([^'"`]*)['"`]\s*,\s*['"`]([^'"`]*)['"`]"#).unwrap()
});

// RabbitMQ: channel.consume(queue, callback)
pub static RABBITMQ_CONSUME_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"channel\.consume\s*\(\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// AWS SQS: sqs.sendMessage({ QueueUrl: '...' })
pub static SQS_SEND_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"sqs\.sendMessage\s*\(\s*\{[^}]*QueueUrl:\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// AWS SQS: sqs.receiveMessage({ QueueUrl: '...' })
pub static SQS_RECV_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"sqs\.receiveMessage\s*\(\s*\{[^}]*QueueUrl:\s*['"`]([^'"`]+)['"`]"#).unwrap()
});

// GCP Pub/Sub: pubsub.topic('name').publish(...)
pub static PUBSUB_PUBLISH_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"pubsub\.topic\s*\(\s*['"`]([^'"`]+)['"`]\s*\)\.publish"#).unwrap()
});

// GCP Pub/Sub: pubsub.subscription('name').on('message', ...)
pub static PUBSUB_SUB_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"pubsub\.subscription\s*\(\s*['"`]([^'"`]+)['"`]\s*\)"#).unwrap()
});

// JWT: jwt.sign(payload, secret)
pub static JWT_SIGN_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"jwt\.sign\s*\("#).unwrap()
});

// JWT: jwt.verify / jwt.decode
pub static JWT_VERIFY_RE: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r#"jwt\.(verify|decode)\s*\("#).unwrap()
});
