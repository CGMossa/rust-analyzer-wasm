import init, { WorldState } from '../ra-wasm/pkg/wasm_demo.js';

const start = async () => {
    await init();
    const state = new WorldState();

    onmessage = (e) => {
        const { which, args, id } = e.data;
        try {
            const result = state[which](...args);
            postMessage({ id, result });
        } catch (err) {
            postMessage({ id, error: String(err && err.stack ? err.stack : err) });
        }
    };
};

start().then(() => {
    postMessage({ id: 'ra-worker-ready' });
});
