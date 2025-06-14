use ree::{Engine, RequestCtx,  ResponseBuilder, middleware, MiddlewareFn};
use serde_json::json;
use std::{sync::Arc, time::Instant};

// =============================================================================
// 示例中间件实现 - 演示如何创建各种类型的中间件
// =============================================================================

/// 🚀 访问日志中间件
fn access_log() -> MiddlewareFn {
    middleware(|ctx, next| async move {
        let start = Instant::now();
        let method = ctx.request.method().to_string();
        let path = ctx.request.uri().path().to_string();
        
        let response = next(ctx).await;
        
        println!(
            "{} {} {} {}ms",
            method,
            path,
            response.status().as_str(),
            start.elapsed().as_millis()
        );
        
        response
    })
}

/// 🚀 计时器中间件
fn timer() -> MiddlewareFn {
    middleware(|ctx, next| async move {
        let start = Instant::now();
        let response = next(ctx).await;
        println!("请求处理耗时: {}ms", start.elapsed().as_millis());
        response
    })
}

/// 🚀 认证中间件
fn auth(token: &'static str) -> MiddlewareFn {
    middleware(move |ctx, next| async move {
        if let Some(auth) = ctx.request.headers().get("Authorization") {
            if auth.to_str().unwrap_or("") == token {
                return next(ctx).await;
            }
        }
        ResponseBuilder::unauthorized_json(r#"{"error": "Unauthorized"}"#)
    })
}

/// 🚀 JWT 认证中间件（简化版本，用于演示）
fn jwt_auth(secret: &'static str) -> MiddlewareFn {
    middleware(move |ctx, next| async move {
        // 从 Authorization header 获取 JWT token
        if let Some(auth_header) = ctx.request.headers().get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    // 简化的JWT验证逻辑（实际项目中应使用专业的JWT库如jsonwebtoken）
                    if validate_jwt_token(token, secret) {
                        println!("✅ JWT认证成功: {}", extract_user_from_token(token));
                        return next(ctx).await;
                    }
                }
            }
        }
        
        ResponseBuilder::unauthorized_json(r#"{"error": "Invalid or missing JWT token"}"#)
    })
}

/// 简化的JWT验证函数（仅用于演示）
fn validate_jwt_token(token: &str, _secret: &str) -> bool {
    // 这里是一个简化的验证逻辑
    // 实际项目中应该：
    // 1. 解析JWT的header、payload、signature
    // 2. 验证签名
    // 3. 检查过期时间
    // 4. 验证issuer、audience等claim
    
    // 演示：假设token格式为 "user.role.timestamp"
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() == 3 {
        // 简单检查：用户名不为空，角色有效，时间戳不太旧
        let user = parts[0];
        let role = parts[1];
        let timestamp = parts[2].parse::<u64>().unwrap_or(0);
        let current_time = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_secs();
        
        !user.is_empty() && 
        (role == "admin" || role == "user") && 
        (current_time - timestamp) < 3600 // 1小时内有效
    } else {
        false
    }
}

/// 从JWT token中提取用户信息（简化版本）
fn extract_user_from_token(token: &str) -> String {
    let parts: Vec<&str> = token.split('.').collect();
    if parts.len() == 3 {
        format!("{}({})", parts[0], parts[1])
    } else {
        "unknown".to_string()
    }
}

/// 🚀 JWT 权限检查中间件
fn jwt_require_role(required_role: &'static str) -> MiddlewareFn {
    middleware(move |ctx, next| async move {
        // 这个中间件应该在 jwt_auth 之后使用
        // 从 Authorization header 获取并解析角色
        if let Some(auth_header) = ctx.request.headers().get("Authorization") {
            if let Ok(auth_str) = auth_header.to_str() {
                if let Some(token) = auth_str.strip_prefix("Bearer ") {
                    let parts: Vec<&str> = token.split('.').collect();
                    if parts.len() == 3 {
                        let role = parts[1];
                        if role == required_role || role == "admin" { // admin有所有权限
                            return next(ctx).await;
                        }
                    }
                }
            }
        }
        
        ResponseBuilder::forbidden_json(format!(
            r#"{{"error": "Access denied. Required role: {}"}}"#, 
            required_role
        ))
    })
}

/// 生成简化的JWT token（仅用于演示）
fn generate_demo_jwt_token(user: &str, role: &str) -> String {
    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    format!("{}.{}.{}", user, role, timestamp)
}

/// 🚀 请求计数器中间件
fn request_counter() -> MiddlewareFn {
    let counter = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    middleware(move |ctx, next| {
        let counter = counter.clone();
        async move {
            let current = counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            println!("总请求数: {}", current + 1);
            next(ctx).await
        }
    })
}

/// CORS 中间件构建器
struct CorsBuilder {
    allow_origin: String,
    allow_methods: Vec<String>,
    allow_headers: Vec<String>,
}

impl CorsBuilder {
    fn new() -> Self {
        Self {
            allow_origin: "*".to_string(),
            allow_methods: vec!["GET".to_string(), "POST".to_string(), "PUT".to_string(), "DELETE".to_string(), "OPTIONS".to_string()],
            allow_headers: vec!["Content-Type".to_string(), "Authorization".to_string()],
        }
    }

    fn allow_origin(mut self, origin: &str) -> Self {
        self.allow_origin = origin.to_string();
        self
    }

    fn allow_methods(mut self, methods: &[&str]) -> Self {
        self.allow_methods = methods.iter().map(|s| s.to_string()).collect();
        self
    }

    fn allow_headers(mut self, headers: &[&str]) -> Self {
        self.allow_headers = headers.iter().map(|s| s.to_string()).collect();
        self
    }

    fn build(self) -> MiddlewareFn {
        let origin = self.allow_origin;
        let methods = self.allow_methods.join(", ");
        let headers = self.allow_headers.join(", ");
        
        middleware(move |ctx, next| {
            let origin = origin.clone();
            let methods = methods.clone();
            let headers = headers.clone();
            async move {
                let mut response = next(ctx).await;
                
                let resp_headers = response.headers_mut();
                resp_headers.insert("Access-Control-Allow-Origin", origin.parse().unwrap());
                resp_headers.insert("Access-Control-Allow-Methods", methods.parse().unwrap());
                resp_headers.insert("Access-Control-Allow-Headers", headers.parse().unwrap());
                
                response
            }
        })
    }
}

/// CORS 中间件
fn cors() -> CorsBuilder {
    CorsBuilder::new()
}

/// 🚀 限流中间件（示例）
fn rate_limit(max_requests: usize) -> MiddlewareFn {
    let requests_count = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let last_reset = Arc::new(std::sync::Mutex::new(Instant::now()));
    
    middleware(move |ctx, next| {
        let requests_count = requests_count.clone();
        let last_reset = last_reset.clone();
        async move {
            // 简单的限流实现（每分钟重置）
            {
                let mut last_reset = last_reset.lock().unwrap();
                if last_reset.elapsed().as_secs() > 60 {
                    requests_count.store(0, std::sync::atomic::Ordering::SeqCst);
                    *last_reset = Instant::now();
                }
            }
            
            let current = requests_count.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            if current >= max_requests {
                return ResponseBuilder::too_many_requests_json(r#"{"error": "Rate limit exceeded"}"#);
            }
            
            next(ctx).await
        }
    })
}

/// 🚀 错误处理中间件
fn error_handler() -> MiddlewareFn {
    middleware(|ctx, next| async move {
        // 在调用 next 之前提取需要的信息
        let method = ctx.request.method().to_string();
        let path = ctx.request.uri().path().to_string();
        
        let response = next(ctx).await;
        
        // 如果是错误状态码，添加一些调试信息
        if response.status().is_client_error() || response.status().is_server_error() {
            println!("⚠️ 错误响应: {} for {} {}", 
                response.status(), 
                method, 
                path
            );
        }
        
        response
    })
}

// =============================================================================
// 主应用程序
// =============================================================================

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut app = Engine::new();

    println!("🛠 Ree HTTP Framework - 函数式中间件系统");
    println!("════════════════════════════════════");
    println!("✨ 全新的函数式中间件API，零开销、易组合！");

    // 1. 全局中间件 - 应用到所有路由
    println!("1️⃣ 全局中间件 - 应用到所有路由");
    app.use_middleware(access_log()); // 访问日志
    app.use_middleware(timer()); // 计时器
    app.use_middleware(request_counter()); // 请求计数器

    // 2. CORS 中间件（支持builder模式）
    println!("2️⃣ CORS 中间件");
    app.use_middleware(
        cors()
            .allow_origin("*")
            .allow_methods(&["GET", "POST", "PUT", "DELETE"])
            .allow_headers(&["Content-Type", "Authorization"])
            .build()
    );

    // 3. 错误处理和限流中间件
    println!("3️⃣ 错误处理和限流中间件");
    app.use_middleware(error_handler());
    app.use_middleware(rate_limit(100)); // 每分钟最多100个请求

    // 4. 自定义中间件 - 直接使用 middleware 函数创建
    println!("4️⃣ 自定义中间件");
    
    // 简单的日志中间件
    app.use_middleware(middleware(|ctx, next| async move {
        println!("🔍 处理请求: {} {}", ctx.request.method(), ctx.request.uri().path());
        let response = next(ctx).await;
        println!("✅ 响应状态: {}", response.status());
        response
    }));

    // 5. 路由组中间件
    println!("5️⃣ 路由组中间件");
    {
        let  api_group = app.group("/api");
        
        // 组专用的认证中间件
        api_group.use_middleware(auth("Bearer secret-token"));
        
        // 组专用的限流中间件（更严格）
        api_group.use_middleware(rate_limit(10)); // API组每分钟最多10个请求
        
        // 组专用的请求验证中间件
        api_group.use_middleware(middleware(|ctx, next| async move {
            println!("🚦 API 组: 验证请求格式");
            // 这里可以添加请求格式验证逻辑
            next(ctx).await
        }));

        // API 路由
        api_group.get("/users", |_ctx: RequestCtx| async move {
            json!({
                "users": [
                    {"id": 1, "name": "Alice", "role": "admin"},
                    {"id": 2, "name": "Bob", "role": "user"}
                ],
                "total": 2
            })
        });

        api_group.post("/users", |_ctx: RequestCtx| async move {
            json!({"message": "用户创建成功", "id": 3, "status": "created"})
        });

        api_group.get("/stats", |_ctx: RequestCtx| async move {
            json!({
                "api_version": "v1.0",
                "uptime": "1 day",
                "requests_today": 1234,
                "middleware_chain": [
                    "global: access_log",
                    "global: timer", 
                    "global: request_counter",
                    "global: cors",
                    "global: error_handler",
                    "global: rate_limit(100)",
                    "global: custom_logger",
                    "api_group: auth",
                    "api_group: rate_limit(10)",
                    "api_group: request_validator"
                ]
            })
        });
    }

    // 6. JWT 认证路由组演示
    println!("6️⃣ JWT 认证路由组");
    {
        let mut jwt_group = app.group("/jwt");
        
        // JWT认证中间件
        jwt_group.use_middleware(jwt_auth("my-secret-key"));
        
        // JWT路由
        jwt_group.get("/profile", |_ctx: RequestCtx| async move {
            json!({
                "message": "用户个人资料",
                "user": "从JWT token中解析的用户信息",
                "auth_method": "JWT"
            })
        });

        jwt_group.get("/dashboard", |_ctx: RequestCtx| async move {
            json!({
                "message": "用户仪表板",
                "data": ["图表1", "图表2", "图表3"],
                "auth_method": "JWT"
            })
        });
    }

    // 7. JWT + 角色权限路由组演示
    println!("7️⃣ JWT + 角色权限路由组");
    {
        let mut admin_group = app.group("/admin");
        
        // JWT认证 + 管理员角色要求
        admin_group.use_middleware(jwt_auth("my-secret-key"));
        admin_group.use_middleware(jwt_require_role("admin"));
        
        admin_group.get("/users", |_ctx: RequestCtx| async move {
            json!({
                "message": "管理员：用户列表",
                "users": [
                    {"id": 1, "name": "Alice", "role": "admin"},
                    {"id": 2, "name": "Bob", "role": "user"},
                    {"id": 3, "name": "Charlie", "role": "user"}
                ],
                "auth_method": "JWT + Role"
            })
        });

        admin_group.post("/users", |_ctx: RequestCtx| async move {
            json!({
                "message": "管理员：创建用户成功",
                "auth_method": "JWT + Role"
            })
        });
    }

    // 8. JWT Token生成端点（用于测试）
    app.post("/auth/login", |_ctx: RequestCtx| async move {
        // 在实际项目中，这里应该验证用户名密码
        let admin_token = generate_demo_jwt_token("alice", "admin");
        let user_token = generate_demo_jwt_token("bob", "user");
        
        json!({
            "message": "登录成功（演示）",
            "tokens": {
                "admin": admin_token,
                "user": user_token
            },
            "usage": {
                "header": "Authorization: Bearer <token>",
                "endpoints": {
                    "jwt_protected": "/jwt/profile, /jwt/dashboard",
                    "admin_only": "/admin/users"
                }
            }
        })
    });

    // 9. 基础路由（不需要认证）
    println!("9️⃣ 基础路由（应用全局中间件）");
    
    app.get("/", |_: RequestCtx| async {
        json!({
            "message": "🎉 欢迎使用 Ree HTTP Framework!",
            "version": "0.1.0",
            "features": [
                "函数式中间件",
                "零开销抽象", 
                "易于组合",
                "类型安全",
                "链式执行"
            ],
            "middleware_examples": [
                "访问日志",
                "计时器",
                "请求计数",
                "CORS",
                "简单认证",
                "JWT认证", 
                "角色权限",
                "限流",
                "错误处理"
            ]
        })
    });

    app.get("/health", |_: RequestCtx| async {
        json!({"status": "ok", "timestamp": "2025-06-16T12:00:00Z"})
    });

    app.get("/middleware-test", |_: RequestCtx| async {
        json!({
            "message": "这个响应经过了所有全局中间件处理",
            "middlewares_applied": [
                "access_log",
                "timer", 
                "request_counter",
                "cors",
                "error_handler",
                "rate_limit(100)",
                "custom_logger"
            ]
        })
    });

    // 7. 错误处理演示
    app.get("/error", |_: RequestCtx| async {
        json!({"error": "内部服务器错误", "code": 500})
    });

    app.get("/not-found", |_: RequestCtx| async {
        json!({"error": "资源未找到", "code": 404})
    });

    println!("\n🚀 服务器启动中...");
    println!("📍 地址: http://127.0.0.1:3000");
    println!("\n📋 测试路由:");
    println!("  GET  /                  - 首页");
    println!("  GET  /health            - 健康检查");
    println!("  GET  /middleware-test   - 中间件测试");
    println!("  GET  /error             - 错误处理演示");
    println!("  GET  /not-found         - 404错误演示");
    println!("  GET  /api/users         - 需要认证 (Bearer secret-token)");
    println!("  POST /api/users         - 需要认证 (Bearer secret-token)");
    println!("  GET  /api/stats         - API统计信息");
    println!("  POST /auth/login        - 获取JWT token（演示）");
    println!("  GET  /jwt/profile       - JWT认证用户信息");
    println!("  GET  /jwt/dashboard     - JWT认证仪表板");
    println!("  GET  /admin/users       - 管理员用户列表（需要admin角色）");
    println!("  POST /admin/users       - 管理员创建用户（需要admin角色）");
    println!("\n💡 测试简单认证API:");
    println!("  curl -H 'Authorization: Bearer secret-token' http://127.0.0.1:3000/api/users");
    println!("\n🔐 测试JWT认证:");
    println!("  1. 获取token: curl -X POST http://127.0.0.1:3000/auth/login");
    println!("  2. 使用token: curl -H 'Authorization: Bearer <admin_token>' http://127.0.0.1:3000/jwt/profile");
    println!("  3. 管理员API: curl -H 'Authorization: Bearer <admin_token>' http://127.0.0.1:3000/admin/users");
    println!("  4. 普通用户API: curl -H 'Authorization: Bearer <user_token>' http://127.0.0.1:3000/jwt/dashboard");
    println!("\n🔍 测试限流:");
    println!("  快速发送多个请求观察限流效果");

    app.run("127.0.0.1:3000").await?;
    Ok(())
}