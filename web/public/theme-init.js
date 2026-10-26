try {
  var theme = JSON.parse(localStorage.getItem('theme'))
  document.documentElement.classList.add(theme === 'light' ? 'light' : 'dark')
} catch (e) {
  document.documentElement.classList.add('dark')
}
