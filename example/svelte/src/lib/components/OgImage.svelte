<script lang="ts">
  import logo from "../../../../../assets/images/takumi.svg";

  let { name = "Goo goo gaga" } = $props();
</script>

<svelte:options css="injected" />

<div id="og-image" class="text-7xl font-semibold text-black w-full h-full flex items-center justify-center p-12 flex-col gap-4">
  <img src={logo} alt="Takumi Logo" width="128" /> 
  <div class="text-center whitespace-pre-wrap leading-normal">
    Welcome {name} to
    <span class="text-svelte font-bold">Svelte</span> world 🔥
  </div>
</div>

<style>
  #og-image {
    background-image: linear-gradient(to bottom right, var(--color-orange-50), var(--color-red-200));
  }
</style>
