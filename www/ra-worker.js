import init, { WorldState } from '../ra-wasm/pkg/wasm_demo.js';

const start = async () => {
    await init();

    const state = new WorldState();
    
    onmessage = (e) => {
        const { which, args, id } = e.data;
        const result = state[which](...args);

        postMessage({
            id: id,
            result: result
        });
    };
};

start().then(() => {
    postMessage({
        id: "ra-worker-ready"
    })
})


    
