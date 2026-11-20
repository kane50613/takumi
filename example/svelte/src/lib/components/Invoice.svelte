<script lang="ts">
  let { id, items }: { id: number; items: { name: string; price: number }[] } = $props();

  const total = $derived(items.reduce((sum, item) => sum + item.price, 0));
</script>

<svelte:options css="injected" />

<main>
  <h1>Invoice {id}</h1>
  <table>
    <tbody>
      {#each items as item}
        <tr><td>{item.name}</td><td>${item.price}</td></tr>
      {/each}
    </tbody>
  </table>
  <p>Total: ${total}</p>
</main>

<style>
  main {
    font-family: Geist;
    padding: 48px;
  }

  table {
    width: 100%;
    border-collapse: collapse;
  }

  td {
    padding: 8px 0;
    border-bottom: 1px solid #e5e5e5;
  }

  td:last-child {
    text-align: right;
  }
</style>
