const mineMap = document.getElementById('mine-map');

let col = 9;
let row = 9;
let mines = 10;

const easy = () => {
  for(let i = 0; i < col; i++){
    const div = document.createElement('div');
    mineMap.appendChild(div);
    for(let j = 0; j < row; j++){
      const div = document.createElement('div');
      mineMap.appendChild(div);
    }
  }
}

easy();