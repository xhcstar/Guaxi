use crate::guaxiservices::chrome_extension_service;
use crate::guaxiservices::markdown_image_process;
use crate::state::SharedState;
use actix_cors::Cors;
use actix_web::web::{Data, JsonConfig, PayloadConfig}; // 添加 Data 导入
use actix_web::{App, HttpServer};

pub async fn start_backend_services(state: SharedState) -> std::io::Result<()> {
    let server = HttpServer::new(move || {
        // 添加CORS配置
        let cors = Cors::default()
            .allow_any_origin()
            .allowed_methods(vec!["GET", "POST"])
            .allow_any_header()
            .max_age(3600);

        App::new()
            .app_data(Data::new(state.clone())) // 使用正确的 Data 类型
            .wrap(cors) // 应用CORS中间件
            .configure(markdown_image_process::config)
            .configure(|cfg| chrome_extension_service::config(cfg, Data::new(state.clone()))) // 使用正确的 Data 类型
            .app_data(JsonConfig::default().limit(10 * 1024 * 1024)) // 10MB限制
            .app_data(PayloadConfig::default().limit(10 * 1024 * 1024)) // 10MB限制
    })
    .bind("127.0.0.1:8888")?;

    server.run().await
}
