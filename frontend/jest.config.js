const nextJest = require('next/jest');

/**
 * Next.js용 Jest 설정을 생성하는 함수
 * @param {Object} config - 기본 설정 객체
 * @property {string} dir - Next.js 프로젝트 루트 디렉토리
 */
const createJestConfig = nextJest({
  dir: './', // Next.js 프로젝트 루트 위치
});

/** 
 * Jest 설정 객체
 * @type {import('jest').Config}
 */
const config = {
  setupFilesAfterEnv: ["<rootDir>/src/setupTests.ts"],
  // jsdom 환경에서 테스트 실행 (브라우저 API 시뮬레이션)
  testEnvironment: 'jest-environment-jsdom',
  // 테스트 대상에서 제외할 경로
  modulePathIgnorePatterns: ['<rootDir>/.next/', '<rootDir>/node_modules/'],
  // 테스트 파일 패턴
  testMatch: ['**/*.test.tsx', '**/*.test.ts'],
  // 모듈 별칭 설정 (tsconfig.json의 paths와 일치시킴)
  moduleNameMapper: {
    // @ 경로 별칭을 src 폴더로 매핑
    '^@/(.*)$': '<rootDir>/src/$1',
  },
};

module.exports = createJestConfig(config);