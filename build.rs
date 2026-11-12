fn main() {
    // Fontes embutidas no binário (EmbedFiles): o executável não depende do caminho de ui/.
    let config = slint_build::CompilerConfiguration::new()
        .embed_resources(slint_build::EmbedResourcesKind::EmbedFiles);
    slint_build::compile_with_config("ui/main.slint", config)
        .expect("falha ao compilar a interface Slint");
}
