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

  // ── Canvas CRT: fondo + estática de transición ──────────────────────
  // La onda idle NO se pinta en canvas: es un SVG con animación CSS (la
  // mueve el compositor, no JS). TODO lo demás del canvas es idéntico a
  // la lógica de 55b5b5f: el canvas pinta el fondo y el ruido de la
  // ventana de transición, y NUNCA se oculta (si se oculta, el
  // IntersectionObserver lo marca invisible y la estática no se dibuja).
  let canvas_ref = $state(null);
  let ctx = $state(null);
  let animation_id = null;
  let colors = {};

  const update_colors = () => {
    colors = {
      back: get_colors('--color-back', '#0d0d0d'),
      accent: get_colors('--color-accent', '#00ffdd'),
      detail: get_colors('--color-accent-detail', '#ffffff'),
    };
    needs_background = true;
  };

  let is_visible = true;

  const FPS = 30;
  let last_frame = 0;
  // El canvas se repinta solo cuando hace falta (montar, resize, cambio de
  // tema, fin de la transición): en idle no hay repintado por frame.
  let needs_background = true;
  let was_active = false;

  // Onda idle: 3 períodos de 100 unidades sobre un viewBox de 200 → al
  // desplazar un período siempre hay trazo bajo la ventana visible.
  const WAVE_PATH = (() => {
    const pts = [];
    for (let x = 0; x <= 300; x += 2) {
      const y = 50 + 15 * Math.sin((x / 100) * Math.PI * 2);
      pts.push(`${x},${y.toFixed(2)}`);
    }
    return `M${pts.join("L")}`;
  })();

  let show_wave = $derived(!current && !window_active);

  // Fondo del monitor (el canvas es la única fuente del color: idle y
  // estática comparten el MISMO fondo, como antes).
  function paint_background() {
    if (!ctx || !canvas_ref) return;
    const dpr = devicePixelRatio || 1;
    ctx.setTransform(dpr, 0, 0, dpr, 0, 0);
    ctx.fillStyle = colors.back;
    ctx.fillRect(0, 0, canvas_ref.width / dpr, canvas_ref.height / dpr);
    needs_background = false;
  }

  function draw_frame(timestamp) {
    if (!is_visible) {
      animation_id = requestAnimationFrame(draw_frame);
      return;
    }

    if (timestamp - last_frame < 1000 / FPS) {
      animation_id = requestAnimationFrame(draw_frame);
      return;
    }
    last_frame = timestamp;

    const dpr = devicePixelRatio || 1;
    const iw = canvas_ref.width;
    const ih = canvas_ref.height;

    if (window_active) {
      was_active = true;
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
      // Terminó la ventana de transición: el canvas quedó con el ruido,
      // se repinta el fondo (una vez).
      if (was_active) {
        was_active = false;
        needs_background = true;
      }
      if (needs_background) paint_background();
    }

    animation_id = requestAnimationFrame(draw_frame);
  }

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
      // resizear el canvas limpia su bitmap → hay que repintar el fondo
      needs_background = true;
    });
    ro.observe(canvas_ref.parentElement);

    const observer = new IntersectionObserver(([entry]) => {
      is_visible = entry.isIntersecting;
    });
    observer.observe(canvas_ref);

    ctx = canvas_ref.getContext("2d");

    // El loop corre siempre (como en 55b5b5f): así window_active se
    // chequea en cada frame y la estática aparece en los mismos casos
    // que antes. Solo el repintado se hace cuando hace falta.
    animation_id = requestAnimationFrame(draw_frame);

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
    <div class="w-full h-full bg-back relative">
      <!-- Onda idle: SVG con animación CSS (la mueve el compositor, no JS).
           opacity (y no hidden) para que la animación no se reinicie. -->
      <svg
        class="absolute inset-0 w-full h-full pointer-events-none transition-opacity duration-100
               {show_wave ? 'opacity-100' : 'opacity-0'}"
        viewBox="0 0 200 100"
        preserveAspectRatio="none"
        aria-hidden="true"
      >
        <path
          d={WAVE_PATH}
          fill="none"
          stroke-width="2"
          vector-effect="non-scaling-stroke"
          style="stroke: var(--color-accent-detail);"
          class="wave-path"
        />
      </svg>

      <!-- El canvas NUNCA se oculta: si se oculta, el IntersectionObserver
           lo marca invisible y la estática deja de dibujarse. -->
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

<style>
  /* Un período de la onda por vuelta: el trazo es continuo (el path
     cubre 3 períodos), así que el loop no se nota. */
  .wave-path {
    animation: wave-scroll 1.3s linear infinite;
  }

  @keyframes wave-scroll {
    from {
      transform: translateX(0);
    }
    to {
      transform: translateX(-100px);
    }
  }
</style>
