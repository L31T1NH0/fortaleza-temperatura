#[cfg(target_arch = "wasm32")]
mod app;
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
mod model;

#[cfg(target_arch = "wasm32")]
#[wasm_bindgen::prelude::wasm_bindgen(start)]
pub fn start() {
    console_error_panic_hook::set_once();
    app::run().expect("Não foi possível iniciar o painel Iced");
}
