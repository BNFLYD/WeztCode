import adapter from '@sveltejs/adapter-static';

/** @type {import('@sveltejs/kit').Config} */
const config = {
	kit: {
		paths: {
			base: '.'
		},
		adapter: adapter({
			pages: 'dist',
			assets: 'dist',
			fallback: 'index.html',
			precompress: false,
			strict: true
		}),
		prerender: {
			handleHttpError: ({ path, referrer, message }) => {
				// Ignorar errores de favicon y otros recursos no críticos
				if (path.includes('favicon')) return;
				throw new Error(message);
			}
		}
	}
};

export default config;
