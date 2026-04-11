ctx => ({
  type: 'workspace',
  children: [
    { type: 'window', id: 'main', match: 'app_id="firefox"' },
    { type: 'slot', id: 'rest', class: ['rest'] }
  ]
})
