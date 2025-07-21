module.exports = {
  parser: '@typescript-eslint/parser',
  extends: [
    'next',
    'next/core-web-vitals',
    'plugin:@typescript-eslint/recommended',
    'plugin:prettier/recommended',
  ],
  plugins: ['@typescript-eslint', 'prettier'],
  rules: {
    'prettier/prettier': 'error',
    'import/no-anonymous-default-export': 'off',  // 익명 함수 사용 허용
    'prefer-const': 'off',  // let 대신 const 사용 규칙 비활성화
    // 다른 규칙들을 여기에 추가할 수 있습니다.
  },
};