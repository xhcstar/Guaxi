use actix_web::{web, App, HttpServer};
use actix_cors::Cors;
use crate::guaxiservices::markdown_image_process;

pub async fn start_backend_services() -> std::io::Result<()> {
    let server = HttpServer::new(move || {
        // 添加CORS配置
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allow_any_header()
            .max_age(3600);

        App::new()
            .wrap(cors)  // 应用CORS中间件
            .configure(markdown_image_process::config)
    })
    .bind("127.0.0.1:8888")?;
    
    server.run().await
}