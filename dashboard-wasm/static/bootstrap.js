import init from '/assets/iced.js';
try { await init({module_or_path:'/assets/iced_bg.wasm'}); }
catch (error) { console.error(error); document.body.textContent = 'Não foi possível iniciar o painel Iced. Recarregue a página.'; }
