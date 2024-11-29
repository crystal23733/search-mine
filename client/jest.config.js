module.exports = {
  preset: 'ts-jest',
  testEnvironment: 'jsdom',
  setupFilesAfterEnv: ['<rootDir>/jest.setup.ts'],
  moduleNameMapper: {
    // CSS 모듈 모킹
    '\\.(css|less|scss|sass)$': 'identity-obj-proxy',
  },
};