import { createPinia } from 'pinia';
import { createApp } from 'vue';
import { restoreCachedThemeToDocument } from '@/lib/utils';
import '@/styles/tokens.css';
import App from './App.vue';

restoreCachedThemeToDocument();

const app = createApp(App);
app.use(createPinia());
app.mount('#app');
