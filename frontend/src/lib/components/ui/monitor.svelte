<script>
  import { onMount } from "svelte";
  import { get_colors } from "$lib/theme";
  import { on_monitor_event } from "./monitor_bus";
  import { get_active_section, SECTIONS } from "$lib/sections";

  // ── Scheduler de transiciones ──────────────────────────────────────
  // Regla maestra: cada cambio de estado en pantalla = exactamente UNA
  // ventana de estática de TRANSITION_MS → nuevo estado. Las ventanas
  // nunca se encadenan: los cambios que llegan durante la ventana
  // reemplazan el destino (así un close seguido de open coalescea en
  // una sola transición; el idle intermedio jamás se renderiza). Los
  // estados vivos de la misma vista (misma key, p. ej. talking↔thinking
  // del mismo agente) se actualizan en caliente, sin estática.

  const TRANSITION_MS = 200;

  // Vista renderizada: { key, component, props } | null (null = idle: onda)
  let current = $state(null);
  // Destino de la ventana activa (estado interno, no se renderiza)
  let pending = null;
  let window_active = $state(false);
  let transition_timer = null;

  let label_id = $state(get_active_section());

  const same_view = (a, b) =>
    a === b || (a !== null && b !== null && a.key === b.key);

  function open_window(target) {
    pending = target;
    window_active = true;
    if (transition_timer) clearTimeout(transition_timer);
    transition_timer = setTimeout(() => {
      current = pending;
      pending = null;
      window_active = false;
      transition_timer = null;
    }, TRANSITION_MS);
  }

  function apply_view(view) {
    if (window_active) {
      // Coalescing: el último estado gana; se renderiza al terminar la
      // ventana en curso. Jamás se abre una ventana dentro de otra.
      if (pending && same_view(pending, view)) {
        pending = { ...pending, props: view.props };
      } else {
        pending = view;
      }
      return;
    }
    if (same_view(current, view)) {
      // Estado vivo de la misma vista: swap en caliente, sin estática.
      if (current) current = { ...current, props: view.props };
      return;
    }
    open_window(view);
  }

  function handle_monitor_event(e) {
    if (e.type === "section") {
      label_id = e.id;
      if (window_active) {
        pending = null;
        return;
      }
      // Cambiar de sección SIEMPRE cruje (aunque la pantalla quede en
      // idle): el monitor "cambia de canal". Si la sección entrante
      // señaliza su canal dentro de la ventana, coalescea en la misma.
      open_window(null);
    } else if (e.type === "view") {
      apply_view(e.view);
    }
  }

  onMount(() => on_monitor_event(handle_monitor_event));

  // ── Canvas CRT: onda idle + estática de transición ──────────────────
  let canvas_ref = $state(null);
  let animation_id = null;
  let colors = {};

  const update_colors = () => {
    colors = {
      back: get_colors('--color-back', '#0d0d0d'),
      accent: get_colors('--color-accent', '#00ffdd'),
      detail: get_colors('--color-accent-detail', '#ffffff'),
    };
  };

  let is_visible = true;

  const FPS = 30;
  let last_frame = 0;
  let start_time = 0;

  onMount(() => {
    if (!canvas_ref) return;
    update_colors();

    const dpr = devicePixelRatio || 1;
    const rect = canvas_ref.parentElement.getBoundingClientRect();
    canvas_ref.width = rect.width * dpr;
    canvas_ref.height = rect.height * dpr;

    window.addEventListener('theme-change', update_colors);

    const ro = new ResizeObserver(() => {
      const dpr = devicePixelRatio || 1;
      const rect = canvas_ref.parentElement.getBoundingClientRect();
      canvas_ref.width = rect.width * dpr;
      canvas_ref.height = rect.height * dpr;
    });
    ro.observe(canvas_ref.parentElement);

    const observer = new IntersectionObserver(([entry]) => {
      is_visible = entry.isIntersecting;
    });
    observer.observe(canvas_ref);

    const ctx = canvas_ref.getContext("2d");

    const animate = (timestamp) => {
      if (!is_visible) {
        animation_id = requestAnimationFrame(animate);
        return;
      }

      if (timestamp - last_frame < 1000 / FPS) {
        animation_id = requestAnimationFrame(animate);
        return;
      }
      last_frame = timestamp;

      if (!start_time) start_time = timestamp;
      const elapsed = timestamp - start_time;

      const dpr = devicePixelRatio || 1;
      const iw = canvas_ref.width;
      const ih = canvas_ref.height;

      if (window_active) {
        // Estática de transición: rayas blancas horizontales (mismo look
        // de siempre, ahora con fillRect en vez de re-asignar un
        // ImageData completo por frame) + lavado blanco ocasional.
        // Se dibuja en espacio de device px (mismo cálculo original).
        ctx.setTransform(1, 0, 0, 1, 0, 0);
        ctx.fillStyle = colors.back;
        ctx.fillRect(0, 0, iw, ih);

        ctx.fillStyle = "#ffffff";
        for (let i = 0; i < 100; i++) {
          const x = Math.random() * iw;
          const y = Math.random() * ih;
          const w = Math.random() * 20 * dpr + 5 * dpr;
          ctx.fillRect(x, y, w, 1);
        }

        if (Math.random() < 0.1) {
          ctx.fillStyle = `rgba(255, 255, 255, ${Math.random() * 0.3 + 0.1})`;
          ctx.fillRect(0, 0, iw, ih);
        }
      } else {
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
        const css_w = iw / dpr;
        const css_h = ih / dpr;

        ctx.fillStyle = colors.back;
        ctx.fillRect(0, 0, css_w, css_h);

        // Onda senoidal idle: la pantalla del CRT cuando no hay canal.
        if (!current) {
          ctx.strokeStyle = colors.detail;
          ctx.lineWidth = 2;
          ctx.beginPath();

          const amplitude = css_h * 0.15;
          const frequency = 0.04;
          const center_y = css_h / 2;

          for (let x = 0; x < css_w; x++) {
            const y = center_y + Math.sin((x + elapsed * 0.12) * frequency) * amplitude;
            if (x === 0) {
              ctx.moveTo(x, y);
            } else {
              ctx.lineTo(x, y);
            }
          }
          ctx.stroke();
        }
      }

      animation_id = requestAnimationFrame(animate);
    };

    animate();

    return () => {
      if (animation_id) cancelAnimationFrame(animation_id);
      window.removeEventListener('theme-change', update_colors);
      ro.disconnect();
      observer.disconnect();
    };
  });
</script>

<div class="w-full h-full relative rounded-2xl border border-accent-detail/40 bg-accent-detail">
  <div class="aspect-square m-1 relative rounded-[14px] bg-accent-detail overflow-hidden">
    <div class="w-full h-full bg-back">
      <canvas
        bind:this={canvas_ref}
        class="w-full h-full"
      ></canvas>

      {#if current && !window_active}
        <div class="absolute inset-0 z-10">
          <current.component {...current.props} />
        </div>
      {/if}

      <div
        class="absolute inset-0 pointer-events-none z-30"
        style="background: repeating-linear-gradient(
          0deg,
          rgba(255, 255, 255, 0.09) 0px,
          rgba(255, 255, 255, 0.09) 0px,
          transparent 1px,
          transparent 6px
        )"
      ></div>

      <div
        class="absolute inset-0 pointer-events-none z-30"
        style="background: radial-gradient(ellipse at center, transparent 70%, rgba(0, 0, 0, 0.2) 100%)"
      ></div>

      <div
        class="absolute bottom-2 right-2 text-xs font-specs text-accent-detail"
      >
        {SECTIONS.find((s) => s.id === label_id)?.monitor_label}
      </div>
    </div>
  </div>
</div>
