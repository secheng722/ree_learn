use std::{collections::HashMap, convert::Infallible, hash::Hash, net::SocketAddr, sync::Arc};

use async_trait::async_trait;
use http_body_util::{BodyExt, Empty, Full, combinators::BoxBody};
use hyper::{body::Bytes, server::conn::http1, service::service_fn};
use hyper_util::rt::TokioIo;

use crate::{context::RequestCtx, router::Router};

type HyperResponse = hyper::Response<BoxBody<Bytes, hyper::Error>>;

pub type Response = HyperResponse;

pub struct ResponseBuilder;

fn full<T: Into<Bytes>>(chunk: T) -> BoxBody<Bytes, hyper::Error> {
    Full::new(chunk.into())
        .map_err(|never| match never {})
        .boxed()
}

impl ResponseBuilder {
    pub fn with_text<T: Into<Bytes>>(chunk: T) -> Response {
        hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .header("Content-Type", "text/plain")
            .body(full(chunk))
            .unwrap()
    }

    pub fn empty() -> Response {
        hyper::Response::builder()
            .status(hyper::StatusCode::OK)
            .body(
                Empty::<Bytes>::new()
                    .map_err(|never| match never {}) // 处理空错误类型
                    .boxed(),
            ) // 转换为BoxBody类型)
            .unwrap()
    }
}

#[async_trait]
pub trait Handler: Send + Sync + 'static {
    async fn handle(&self, ctx: RequestCtx) -> Response;
}

#[async_trait]
impl<F: Send + Sync + 'static, Fut> Handler for F
where
    F: Fn(RequestCtx) -> Fut,
    Fut: std::future::Future<Output = Response> + Send + 'static,
{
    async fn handle(&self, ctx: RequestCtx) -> Response {
        (self)(ctx).await
    }
}

pub struct RouterGroup {
    prefix: String,
    router: Router,
    middlewares: Vec<Box<dyn Handler>>,
}

impl RouterGroup {
    pub fn add_route(&mut self, method: &str, pattern: &str, handler: impl Handler) {
        let handler = Box::new(handler);
        self.router.add_route(method, pattern, handler);
    }

    pub fn get(&mut self, path: &str, handler: impl Handler) {
        self.add_route("GET", path, handler);
    }

    pub fn use_middleware(&mut self, middleware: impl Handler) {
        let middleware = Box::new(middleware);
        self.middlewares.push(middleware);
    }
}

pub struct Engine {
    // 不属于任何路由组的路由
    router: Router,
    group: HashMap<String, RouterGroup>,
}

impl Engine {
    pub fn new() -> Self {
        Engine {
            router: Router::new(),
            group: HashMap::new(),
        }
    }

    pub fn group(&mut self, prefix: &str) -> &mut RouterGroup {
        let group = RouterGroup {
            prefix: prefix.to_string(),
            router: Router::new(),
            middlewares: Vec::new(),
        };
        self.group.insert(prefix.to_string(), group);
        self.group.get_mut(prefix).unwrap()
    }

    pub fn add_route(&mut self, method: &str, pattern: &str, handler: impl Handler) {
        let handler = Box::new(handler);
        self.router.add_route(method, pattern, handler);
    }

    pub fn get(&mut self, path: &str, handler: impl Handler) {
        self.add_route("GET", path, handler);
    }

    //self 所有权转移
    pub async fn run(self, addr: &str) -> Result<(), Box<dyn std::error::Error>> {
        let addr = addr.parse::<SocketAddr>()?;
        let listener = tokio::net::TcpListener::bind(addr).await?;
        let router = Arc::new(self.router);
        loop {
            let (stream, _) = listener.accept().await?;
            let io = TokioIo::new(stream); // 将TCP流转换为Tokio的IO接口
            let router = Arc::clone(&router); // 克隆路由表的Arc指针以在新任务中使用
            tokio::task::spawn(async move {
                // 启动一个新的异步任务来处理连接
                if let Err(err) = http1::Builder::new()
                    .serve_connection(
                        // 使用HTTP/1协议服务此连接
                        io,
                        service_fn(move |req| {
                            // 创建服务函数来处理每个HTTP请求
                            let router = Arc::clone(&router); // 再次克隆路由表以在请求处理闭包中使用
                            async move { Ok::<_, Infallible>(router.handle_request(req).await) }
                        }),
                    )
                    .await
                {
                    eprintln!("Error handling connection: {:?}", err);
                }
            });
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn test_new_group() {
        let mut engine = Engine::new();
        let mut group = engine.group("/api");
        group.prefix = "/1".to_string();
        println!("{:?}", group.prefix);
        println!("{:?}", engine.group.get("/api").unwrap().prefix);
    }
}   