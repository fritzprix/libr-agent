import { Logger } from './src/lib/logger';

console.log('Testing message length limiting...');

// Test configuration
Logger.updateConfig({ maxMessageLength: 50 });
console.log('Max message length set to 50');

// Test formatLogMessage function (private이므로 직접 호출은 못하지만, 실제 로깅에서 확인)
const testMessage = 'A'.repeat(100);
console.log('Test message length:', testMessage.length);
console.log('Should be truncated to 47 chars + "..." = 50 chars total');

// 실제 로깅 함수를 통해 테스트
Logger.debug(testMessage, 'TestContext');
