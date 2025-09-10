#!/usr/bin/env python3
import re
import json

def analyze_error_log():
    # error.txt 파일 읽기
    with open('error.txt', 'r', encoding='utf-8') as f:
        content = f.read()

    print('=== SynapticFlow 에러 로그 분석 ===')
    print('=' * 50)

    # 1. 'Incomplete JSON segment at the end' 오류 찾기
    print('\n1. Incomplete JSON 오류 검색')
    print('-' * 30)

    incomplete_json_pattern = r'Incomplete JSON segment at the end'
    matches = re.findall(incomplete_json_pattern, content)
    print(f'발견된 Incomplete JSON 오류 수: {len(matches)}')

    # 각 오류의 라인 번호와 주변 컨텍스트 찾기
    lines = content.split('\n')
    for i, line in enumerate(lines, 1):
        if 'Incomplete JSON segment at the end' in line:
            print(f'\n=== 오류 위치: 라인 {i} ===')
            # 주변 3줄의 컨텍스트 표시
            start = max(0, i-4)
            end = min(len(lines), i+3)
            for j in range(start, end):
                marker = '>>>' if j+1 == i else '   '
                print(f'{marker} {j+1:3d}: {lines[j]}')

    # 2. MALFORMED_FUNCTION_CALL 오류 찾기
    print('\n\n2. MALFORMED_FUNCTION_CALL 오류 검색')
    print('-' * 30)

    malformed_pattern = r'MALFORMED_FUNCTION_CALL'
    malformed_matches = re.findall(malformed_pattern, content)
    print(f'발견된 MALFORMED_FUNCTION_CALL 오류 수: {len(malformed_matches)}')

    # 각 MALFORMED_FUNCTION_CALL의 라인 번호 찾기
    for i, line in enumerate(lines, 1):
        if 'MALFORMED_FUNCTION_CALL' in line:
            print(f'\n=== MALFORMED_FUNCTION_CALL 위치: 라인 {i} ===')
            start = max(0, i-2)
            end = min(len(lines), i+3)
            for j in range(start, end):
                marker = '>>>' if j+1 == i else '   '
                print(f'{marker} {j+1:3d}: {lines[j]}')

    # 3. 프롬프트 토큰 수 분석
    print('\n\n3. 프롬프트 토큰 수 분석')
    print('-' * 30)

    token_pattern = r'"promptTokenCount":(\d+)'
    token_matches = re.findall(token_pattern, content)
    print(f'발견된 promptTokenCount 값들: {token_matches}')

    for match in token_matches:
        token_count = int(match)
        if token_count > 15000:
            print(f'🚨 매우 높은 토큰 수: {token_count} (문제 발생 가능성 높음)')
        elif token_count > 10000:
            print(f'⚠️  높은 토큰 수: {token_count} (주의 필요)')
        else:
            print(f'✓ 정상 토큰 수: {token_count}')

    # 4. JSON 구조 분석
    print('\n\n4. JSON 구조 분석')
    print('-' * 30)

    # chunk 데이터에서 JSON 파싱 시도
    chunk_pattern = r'"chunk"\s*:\s*({[^}]*})'
    chunk_matches = re.findall(chunk_pattern, content, re.DOTALL)

    print(f'발견된 chunk JSON 객체 수: {len(chunk_matches)}')

    for i, chunk_json in enumerate(chunk_matches[:2]):  # 처음 2개만 분석
        print(f'\n--- Chunk {i+1} 분석 ---')
        try:
            # JSON 파싱 시도
            chunk_data = json.loads(chunk_json)
            print('✓ 유효한 JSON 구조')

            # 주요 필드 분석
            if 'candidates' in chunk_data:
                candidates = chunk_data['candidates']
                if candidates and len(candidates) > 0:
                    finish_reason = candidates[0].get('finishReason', 'N/A')
                    print(f'  Finish Reason: {finish_reason}')

            if 'usageMetadata' in chunk_data:
                usage = chunk_data['usageMetadata']
                prompt_tokens = usage.get('promptTokenCount', 0)
                total_tokens = usage.get('totalTokenCount', 0)
                print(f'  Prompt Tokens: {prompt_tokens}')
                print(f'  Total Tokens: {total_tokens}')

        except json.JSONDecodeError as e:
            print(f'✗ JSON 파싱 오류: {e}')
            print(f'  문제 있는 JSON: {chunk_json[:200]}...')

    # 5. 오류 패턴 요약
    print('\n\n5. 오류 패턴 요약')
    print('-' * 30)

    error_patterns = {
        'Incomplete JSON': len(matches),
        'MALFORMED_FUNCTION_CALL': len(malformed_matches),
        'High Token Count (>10k)': sum(1 for m in token_matches if int(m) > 10000),
        'Very High Token Count (>15k)': sum(1 for m in token_matches if int(m) > 15000)
    }

    for pattern, count in error_patterns.items():
        print(f'{pattern}: {count}')

    # 6. 해결 방안 제안
    print('\n\n6. 해결 방안 제안')
    print('-' * 30)

    if error_patterns['Incomplete JSON'] > 0:
        print('• Incomplete JSON 오류:')
        print('  - 프롬프트 길이 축소 고려')
        print('  - 스트리밍 응답 처리 로직 검토')
        print('  - JSON 파싱 타임아웃 설정')

    if error_patterns['MALFORMED_FUNCTION_CALL'] > 0:
        print('• MALFORMED_FUNCTION_CALL 오류:')
        print('  - 함수 호출 JSON 형식 검증')
        print('  - 모델 응답 포맷팅 개선')

    if error_patterns['High Token Count (>10k)'] > 0:
        print('• 높은 토큰 수 문제:')
        print('  - 시스템 프롬프트 최적화')
        print('  - 도구 설명 축소')
        print('  - 컨텍스트 윈도우 관리 개선')

if __name__ == '__main__':
    analyze_error_log()
