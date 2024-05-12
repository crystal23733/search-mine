const body = document.querySelector('body');
const mineMap = document.getElementById('mine-map');

for(let i = 0; i < 60; i++){
  const div = document.createElement('div');
  mineMap.appendChild(div);
}