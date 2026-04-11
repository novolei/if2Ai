<script lang="ts">
  import { invoke } from '@tauri-apps/api/core'
  
  let name: string = 'If2Ai'
  let message: string = ''

  async function greet() {
    message = await invoke('greet', { name })
  }

  async function getVersion() {
    const version = await invoke('get_version')
    message = `App Version: ${version}`
  }
</script>

<main>
  <div class="container">
    <h1>Welcome to {name}</h1>
    <p>AI智能体桌面应用</p>
    
    <div class="input-group">
      <input type="text" bind:value={name} placeholder="Enter name" />
      <button on:click={greet}>Greet</button>
      <button on:click={getVersion}>Get Version</button>
    </div>

    {#if message}
      <p class="message">{message}</p>
    {/if}
  </div>
</main>

<style>
  :global(body) {
    margin: 0;
    padding: 0;
    font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', Roboto, Oxygen, Ubuntu, Cantarell, sans-serif;
    background: linear-gradient(135deg, #667eea 0%, #764ba2 100%);
    min-height: 100vh;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  main {
    width: 100%;
    max-width: 600px;
    padding: 20px;
  }

  .container {
    background: white;
    border-radius: 12px;
    padding: 40px;
    box-shadow: 0 20px 60px rgba(0, 0, 0, 0.3);
    text-align: center;
  }

  h1 {
    margin: 0 0 10px 0;
    color: #333;
  }

  p {
    color: #666;
    margin: 0 0 30px 0;
  }

  .input-group {
    display: flex;
    gap: 10px;
    margin: 20px 0;
  }

  input {
    flex: 1;
    padding: 10px;
    border: 2px solid #ddd;
    border-radius: 6px;
    font-size: 14px;
  }

  input:focus {
    outline: none;
    border-color: #667eea;
  }

  button {
    padding: 10px 20px;
    background: #667eea;
    color: white;
    border: none;
    border-radius: 6px;
    cursor: pointer;
    font-weight: 500;
    transition: background 0.3s;
  }

  button:hover {
    background: #764ba2;
  }

  .message {
    margin-top: 20px;
    color: #667eea;
    font-weight: 500;
  }
</style>
