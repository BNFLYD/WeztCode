// Bridge para comunicación con WebKitGTK backend
// Expone window.weztcode con métodos para comunicarse con Rust

function createBridge() {
  // Verificar si estamos en WebKitGTK (producción) o desarrollo
  const isWebKit = typeof window.webkit !== 'undefined' && window.webkit.messageHandlers;
  
  // Cola de promesas pendientes
  const pendingPromises = new Map();
  let requestId = 0;

  function generateId() {
    return ++requestId;
  }

  function callBackend(command, args = {}) {
    return new Promise((resolve, reject) => {
      const id = generateId();
      pendingPromises.set(id, { resolve, reject });

      const message = JSON.stringify({ id, command, args });

      if (isWebKit && window.webkit.messageHandlers.weztcode) {
        // WebKitGTK nativo
        window.webkit.messageHandlers.weztcode.postMessage(message);
      } else {
        // Fallback para desarrollo (mock)
        console.log('[Bridge Mock]', command, args);
        setTimeout(() => {
          pendingPromises.get(id)?.resolve(`Mock: ${command}`);
          pendingPromises.delete(id);
        }, 100);
      }
    });
  }

  // Exponer respuesta del backend
  window.weztcodeResponse = (id, result, error) => {
    const promise = pendingPromises.get(id);
    if (!promise) return;

    if (error) {
      promise.reject(error);
    } else {
      promise.resolve(result);
    }
    pendingPromises.delete(id);
  };

  return {
    spawnTerm: () => callBackend('spawn_term'),
    listPanes: () => callBackend('list_panes'),
    sendText: (text) => callBackend('send_text', { text }),
    setSize: (width, height) => callBackend('set_size', { width, height }),
    setPosition: (x, y) => callBackend('set_position', { x, y }),
  };
}

// Inicializar bridge global
window.weztcode = createBridge();
