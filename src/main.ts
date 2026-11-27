import "element-plus/dist/index.css";
import "./styles/base.css";

void Promise.all([
  import("vue"),
  import("pinia"),
  import("element-plus"),
  import("./app/App.vue")
]).then(([{ createApp }, { createPinia }, { default: ElementPlus }, { default: App }]) => {
  createApp(App).use(createPinia()).use(ElementPlus).mount("#app");
});