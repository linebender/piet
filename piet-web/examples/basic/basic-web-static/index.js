const rust = import('./dist/piet_web_example');

rust
  .then(m => m.run())
  .catch(console.error);
