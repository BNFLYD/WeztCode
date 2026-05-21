<script>
  import Icon from "@iconify/svelte";

  export let active_section;
  export let on_section_change;

  const sections = [
    { id: "explorer", icon: "solar:code-2-bold", label: "ARCHIVOS" },
    { id: "chat", icon: "mingcute:chat-4-line", label: "CHAT" },
    { id: "term", icon: "devicon-plain:bash", label: "TERMINAL" },
    { id: "view", icon: "streamline-plump:web", label: "VISUALIZAR" },
    { id: "git", icon: "mynaui:git-commit", label: "GIT" },
    { id: "settings", icon: "hugeicons:settings-03", label: "CONFIGURACION" },
  ];
</script>

<div
  class="border border-accent-detail rounded-xl bg-back-deep flex flex-col py-2 px-0 mb-5 mx-4"
>
  <div class="flex w-full justify-between px-4 py-2">
    {#each sections as section}
      <button
        class="isolate bg-accent-detail bg-inner-glow relative w-9 h-9 flex items-center justify-center rounded-md transition-all shadow-[inset_0_1px_0_rgba(255,255,255,0.10),0_2px_6px_rgba(0,0,0,0.25)] hover:shadow-[inset_0_1px_0_rgba(255,255,255,0.20),0_3px_8px_rgba(0,0,0,0.35)] active:shadow-[inset_0_4px_12px_rgba(0,0,0,0.55),inset_0_-1px_0_rgba(255,255,255,0.12)] hover:translate-y-[1px] active:translate-y-[3px] hover:bg-accent-detail/75 {active_section ===
          section.id
            ? 'btn-glow text-back'
            : 'text-back-deep'}"
        on:click={() => on_section_change(section.id)}
      >
        <Icon icon={section.icon} class="w-6 h-6" />
      </button>
    {/each}
  </div>
  <div class="flex justify-center -mb-5">
    <span
      class="bg-back-deep px-2 text-sm text-print-tag font-sans font-bold tracking-[3px]"
    >
      {sections.find((s) => s.id === active_section)?.label}
    </span>
  </div>
</div>
