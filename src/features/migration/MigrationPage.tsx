import type { FC } from 'react';
import { useState } from 'react';
import { useTranslation } from 'react-i18next';
import { useNavigate } from 'react-router-dom';
import { toast } from 'sonner';
import {
  ArrowLeft,
  ArrowUpFromLine,
  ArrowDownToLine,
  ShieldAlert,
  CheckCircle2,
  FolderOpen,
  FileCheck2,
  RefreshCw,
  Info,
  AlertTriangle,
  FolderSync,
} from 'lucide-react';
import {
  Button,
  Card,
  CardContent,
  CardDescription,
  CardFooter,
  CardHeader,
  CardTitle,
  Badge,
  Separator,
} from '@/components/ui';
import { useMigration } from './useMigration';
import type { ConflictStrategy } from '@/lib/backend/migration';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import { useEffect } from 'react';

const MigrationPage: FC = () => {
  const { t } = useTranslation('common');
  const navigate = useNavigate();

  const {
    phase,
    preview,
    exportInfo,
    importResult,
    error,
    progress,
    selectedFilePath,
    selectedExportDir,
    includeSensitiveData,
    setIncludeSensitiveData,
    selectExportFile,
    selectImportFile,
    doInspect,
    doExport,
    doImport,
    doReverifyMcp,
    reset,
  } = useMigration();

  const [strategy, setStrategy] = useState<ConflictStrategy>('skip');
  const [reverifying, setReverifying] = useState(false);

  // Password-based backup states
  const [isExportPasswordOpen, setIsExportPasswordOpen] = useState(false);
  const [exportPassword, setExportPassword] = useState('');
  const [exportPasswordConfirm, setExportPasswordConfirm] = useState('');
  const [exportPasswordError, setExportPasswordError] = useState<string | null>(
    null,
  );

  const [isImportPasswordOpen, setIsImportPasswordOpen] = useState(false);
  const [importPassword, setImportPassword] = useState('');
  const [importPasswordError, setImportPasswordError] = useState<string | null>(
    null,
  );
  const [currentPassword, setCurrentPassword] = useState<string | undefined>(
    undefined,
  );

  // Monitor for password prompt error from the backend
  useEffect(() => {
    if (error === 'PASSWORD_REQUIRED') {
      setIsImportPasswordOpen(true);
      setImportPasswordError(null);
    } else if (error === 'WRONG_PASSWORD') {
      setIsImportPasswordOpen(true);
      setImportPasswordError(
        '비밀번호가 올바르지 않습니다. 다시 입력해주세요.',
      );
    }
  }, [error]);

  const handleCancelImportPassword = () => {
    setIsImportPasswordOpen(false);
    setImportPassword('');
    setImportPasswordError(null);
    reset(); // Reset useMigration state to clear password error
  };

  const handleImportPasswordSubmit = async () => {
    if (!selectedFilePath) return;
    try {
      setImportPasswordError(null);
      await doInspect(selectedFilePath, importPassword);
      setCurrentPassword(importPassword);
      setIsImportPasswordOpen(false);
      setImportPassword('');
      toast.success('백업 파일 비밀번호 확인 완료');
    } catch (e) {
      if (e instanceof Error && e.message === 'WRONG_PASSWORD') {
        setImportPasswordError(
          '비밀번호가 올바르지 않습니다. 다시 입력해주세요.',
        );
      } else {
        setIsImportPasswordOpen(false);
        setImportPassword('');
      }
    }
  };

  const handleExport = async () => {
    if (includeSensitiveData) {
      setExportPassword('');
      setExportPasswordConfirm('');
      setExportPasswordError(null);
      setIsExportPasswordOpen(true);
    } else {
      try {
        await doExport();
        toast.success('데이터를 성공적으로 내보냈습니다!');
      } catch (e) {
        toast.error(
          '내보내기 실패: ' + (e instanceof Error ? e.message : String(e)),
        );
      }
    }
  };

  const handleExportWithPassword = async () => {
    if (exportPassword.length < 4) {
      setExportPasswordError('비밀번호는 최소 4자리 이상이어야 합니다.');
      return;
    }
    if (exportPassword !== exportPasswordConfirm) {
      setExportPasswordError('비밀번호가 일치하지 않습니다.');
      return;
    }

    try {
      setExportPasswordError(null);
      await doExport(exportPassword);
      setIsExportPasswordOpen(false);
      toast.success('암호화된 데이터를 성공적으로 내보냈습니다!');
    } catch (e) {
      setIsExportPasswordOpen(false);
      toast.error(
        '내보내기 실패: ' + (e instanceof Error ? e.message : String(e)),
      );
    }
  };

  const handleImport = async () => {
    try {
      await doImport(strategy, currentPassword);
      toast.success('마이그레이션 데이터를 성공적으로 가져왔습니다!');
    } catch (e) {
      toast.error(
        '가져오기 실패: ' + (e instanceof Error ? e.message : String(e)),
      );
    }
  };

  const handleReverify = async () => {
    setReverifying(true);
    try {
      const results = await doReverifyMcp();
      const failedServers = Object.entries(results)
        .filter(([, status]) => status === 'error')
        .map(([id]) => id);

      if (failedServers.length > 0) {
        toast.warning(`일부 MCP 서버 재검증 실패: ${failedServers.join(', ')}`);
      } else {
        toast.success('MCP 서버 재검증이 완료되었습니다.');
      }
    } catch (e) {
      toast.error(
        '재검증 실패: ' + (e instanceof Error ? e.message : String(e)),
      );
    } finally {
      setReverifying(false);
    }
  };

  const formatBytes = (bytes: number) => {
    if (bytes === 0) return '0 Bytes';
    const k = 1024;
    const sizes = ['Bytes', 'KB', 'MB', 'GB'];
    const i = Math.floor(Math.log(bytes) / Math.log(k));
    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  return (
    <div className="p-6 min-h-full bg-background/50 flex flex-col items-center">
      <div className="max-w-3xl w-full flex flex-col gap-6">
        {/* Header */}
        <div className="flex items-center gap-4">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => navigate('/settings')}
            className="h-10 w-10 rounded-xl"
          >
            <ArrowLeft className="h-5 w-5 text-muted-foreground" />
          </Button>
          <div>
            <h1 className="text-2xl font-bold tracking-tight text-foreground">
              {t('settings.migration.title', '데이터 마이그레이션 (Migration)')}
            </h1>
            <p className="text-sm text-muted-foreground">
              {t(
                'settings.migration.subtitle',
                'LibrAgent 설정, 어시스턴트, 스케줄러 및 커스텀 스킬을 다른 기기로 이전합니다.',
              )}
            </p>
          </div>
        </div>

        {error &&
          error !== 'PASSWORD_REQUIRED' &&
          error !== 'WRONG_PASSWORD' && (
            <div className="p-4 rounded-xl border border-destructive/20 bg-destructive/10 text-destructive-foreground flex gap-3 items-start animate-in fade-in slide-in-from-top-4 duration-300">
              <ShieldAlert className="h-5 w-5 shrink-0 text-destructive mt-0.5" />
              <div className="text-sm flex-1">
                <span className="font-semibold">오류 발생:</span> {error}
              </div>
              <Button
                variant="ghost"
                size="sm"
                onClick={reset}
                className="h-7 text-xs hover:bg-destructive/20 text-destructive-foreground"
              >
                초기화
              </Button>
            </div>
          )}

        {phase === 'idle' && !preview && !exportInfo && !importResult && (
          <div className="grid grid-cols-1 md:grid-cols-2 gap-6">
            {/* Export Card */}
            <Card className="border border-border/80 shadow-md hover:shadow-lg transition-all duration-300 bg-card/40 backdrop-blur-md rounded-2xl flex flex-col h-full">
              <CardHeader className="gap-2">
                <div className="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
                  <ArrowUpFromLine className="h-6 w-6" />
                </div>
                <CardTitle className="text-lg">
                  설정 내보내기 (Export)
                </CardTitle>
                <CardDescription>
                  현재 기기의 전체 설정 및 사용자 스킬 데이터를 단일 아카이브
                  파일(`.libragent-migration`)로 패키징합니다.
                </CardDescription>
              </CardHeader>

              <CardContent className="flex-1 flex flex-col gap-4">
                <div className="rounded-xl border border-warning/20 bg-warning/5 p-4 flex gap-3 items-start">
                  <AlertTriangle className="h-5 w-5 shrink-0 text-warning mt-0.5" />
                  <div className="text-xs text-muted-foreground leading-relaxed">
                    <span className="font-semibold text-foreground">
                      ⚠️ 보안 주의:
                    </span>{' '}
                    API 키와 액세스 토큰은 기본적으로 포함되지 않도록
                    마스킹됩니다. 다른 안전한 채널로 이동하는 경우 아래 옵션을
                    켜십시오.
                  </div>
                </div>

                <div
                  className="flex items-center gap-3 p-3 rounded-xl border bg-background/50 hover:bg-background/80 transition-colors duration-200 cursor-pointer"
                  onClick={() => setIncludeSensitiveData(!includeSensitiveData)}
                >
                  <input
                    type="checkbox"
                    checked={includeSensitiveData}
                    onChange={(e) => setIncludeSensitiveData(e.target.checked)}
                    className="h-4 w-4 rounded border-gray-300 text-primary focus:ring-primary cursor-pointer"
                  />
                  <div className="flex flex-col">
                    <span className="text-xs font-semibold text-foreground">
                      민감한 데이터 포함 (Settings 테이블 전체)
                    </span>
                    <span className="text-[10px] text-muted-foreground">
                      API 키, 액세스 토큰 등을 포함한 settings 테이블 전체의
                      평문 데이터를 백업 아카이브에 동반하여 저장합니다.
                    </span>
                  </div>
                </div>

                <div className="flex flex-col gap-2">
                  <span className="text-xs font-semibold text-muted-foreground">
                    내보낼 경로 선택
                  </span>
                  <div className="flex gap-2">
                    <input
                      type="text"
                      readOnly
                      placeholder="내보낼 폴더를 선택하십시오..."
                      value={selectedExportDir || ''}
                      className="flex-1 px-3 py-2 text-xs rounded-lg border bg-background/30 focus:outline-none truncate"
                    />
                    <Button
                      variant="outline"
                      size="sm"
                      onClick={selectExportFile}
                      className="gap-1.5 h-9 rounded-lg"
                    >
                      <FolderOpen className="h-4 w-4" />
                      선택
                    </Button>
                  </div>
                </div>
              </CardContent>

              <CardFooter className="pt-2">
                <Button
                  onClick={handleExport}
                  disabled={!selectedExportDir}
                  className="w-full gap-2 rounded-xl h-10 shadow-sm"
                >
                  <ArrowUpFromLine className="h-4 w-4" />
                  내보내기 실행
                </Button>
              </CardFooter>
            </Card>

            {/* Import Card */}
            <Card className="border border-border/80 shadow-md hover:shadow-lg transition-all duration-300 bg-card/40 backdrop-blur-md rounded-2xl flex flex-col h-full">
              <CardHeader className="gap-2">
                <div className="h-12 w-12 rounded-xl bg-primary/10 text-primary flex items-center justify-center">
                  <ArrowDownToLine className="h-6 w-6" />
                </div>
                <CardTitle className="text-lg">
                  설정 가져오기 (Import)
                </CardTitle>
                <CardDescription>
                  기존에 내보낸 `.libragent-migration` 아카이브 파일을 불러와
                  현재 환경의 설정을 갱신합니다.
                </CardDescription>
              </CardHeader>

              <CardContent className="flex-1 flex flex-col justify-between">
                <div className="rounded-xl border border-primary/20 bg-primary/5 p-4 flex gap-3 items-start mb-6">
                  <Info className="h-5 w-5 shrink-0 text-primary mt-0.5" />
                  <div className="text-xs text-muted-foreground leading-relaxed">
                    가져오기 전, 데이터 손실 방지를 위해 현재 DB의 **자동
                    백업**이 생성되며 실패 시 즉시 이전 상태로 복구됩니다.
                  </div>
                </div>

                <div className="flex flex-col gap-2">
                  <span className="text-xs font-semibold text-muted-foreground">
                    마이그레이션 아카이브 파일
                  </span>
                  <Button
                    variant="outline"
                    onClick={selectImportFile}
                    className="w-full gap-2 h-14 border-dashed border-2 hover:border-primary/50 hover:bg-primary/5 rounded-xl transition-all duration-200"
                  >
                    <FileCheck2 className="h-5 w-5 text-muted-foreground" />
                    파일 선택 및 유효성 검사
                  </Button>
                </div>
              </CardContent>

              <CardFooter className="pt-2">
                <Button disabled className="w-full gap-2 rounded-xl h-10">
                  <ArrowDownToLine className="h-4 w-4" />
                  가져오기 실행 (파일 분석 대기)
                </Button>
              </CardFooter>
            </Card>
          </div>
        )}

        {/* Loading Phases (inspecting, exporting, importing) */}
        {(phase === 'inspecting' ||
          phase === 'exporting' ||
          phase === 'importing' ||
          phase === 'selecting') && (
          <Card className="border border-border/80 shadow-md bg-card/40 backdrop-blur-md rounded-2xl p-8 flex flex-col items-center justify-center gap-6 text-center animate-in fade-in duration-300">
            <div className="relative flex items-center justify-center">
              <RefreshCw className="h-12 w-12 text-primary animate-spin" />
              <span className="absolute text-[10px] font-bold text-primary">
                {progress}%
              </span>
            </div>
            <div>
              <h3 className="text-lg font-bold text-foreground capitalize">
                {phase === 'inspecting' && '파일 구조 분석 중...'}
                {phase === 'exporting' && '마이그레이션 파일 작성 중...'}
                {phase === 'importing' && '환경 데이터베이스 적용 중...'}
                {phase === 'selecting' && '사용자 입력 대기 중...'}
              </h3>
              <p className="text-sm text-muted-foreground mt-2 max-w-sm">
                {phase === 'inspecting' &&
                  '내용 검사 및 ZIP Slip 방어를 위한 위협 분석을 수행하고 있습니다.'}
                {phase === 'exporting' &&
                  '설정 테이블 직렬화 및 스킬 파일 아카이빙을 진행하고 있습니다.'}
                {phase === 'importing' &&
                  '단일 격리 트랜잭션 내에서 설정을 적용하며 외래키 제약을 검증하고 있습니다.'}
                {phase === 'selecting' &&
                  '폴더/파일 탐색기 창에서 결정을 완료하십시오.'}
              </p>
            </div>
            <div className="w-full max-w-xs bg-muted rounded-full h-1.5 overflow-hidden">
              <div
                className="bg-primary h-full transition-all duration-300"
                style={{ width: `${progress}%` }}
              ></div>
            </div>
          </Card>
        )}

        {/* Inspect Preview Page */}
        {phase === 'idle' && preview && !importResult && (
          <Card className="border border-border/80 shadow-lg bg-card/40 backdrop-blur-md rounded-2xl animate-in fade-in slide-in-from-bottom-4 duration-300">
            <CardHeader className="gap-2">
              <div className="flex justify-between items-start gap-4">
                <div>
                  <CardTitle className="text-lg">
                    마이그레이션 아카이브 파일 분석 완료
                  </CardTitle>
                  <CardDescription className="truncate max-w-lg">
                    {preview.file_path}
                  </CardDescription>
                </div>
                <Badge
                  variant={
                    preview.compatibility === 'Compatible'
                      ? 'default'
                      : typeof preview.compatibility === 'object' &&
                          'NewerVersion' in preview.compatibility
                        ? 'secondary'
                        : 'destructive'
                  }
                  className="rounded-lg py-1 px-2.5 text-xs font-semibold"
                >
                  {preview.compatibility === 'Compatible' && '✅ 호환성 통과'}
                  {typeof preview.compatibility === 'object' &&
                    'NewerVersion' in preview.compatibility &&
                    '⚠️ 상위 버전 경고'}
                  {typeof preview.compatibility === 'object' &&
                    'Incompatible' in preview.compatibility &&
                    '❌ 호환되지 않음'}
                </Badge>
              </div>
            </CardHeader>

            <CardContent className="flex flex-col gap-6">
              {/* Compatibility warning if newer version or incompatible */}
              {typeof preview.compatibility === 'object' && (
                <div className="p-4 rounded-xl border border-warning/20 bg-warning/5 flex gap-3 text-sm text-warning-foreground">
                  <AlertTriangle className="h-5 w-5 shrink-0 text-warning mt-0.5" />
                  <div>
                    <span className="font-bold">호환성 정보:</span>{' '}
                    {'NewerVersion' in preview.compatibility
                      ? preview.compatibility.NewerVersion.message
                      : 'Incompatible' in preview.compatibility
                        ? preview.compatibility.Incompatible.message
                        : ''}
                  </div>
                </div>
              )}

              {/* Archive Info */}
              <div className="grid grid-cols-3 gap-4 text-center rounded-xl bg-background/30 p-4 border">
                <div>
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase">
                    백업 앱 버전
                  </div>
                  <div className="text-sm font-bold text-foreground mt-1">
                    {preview.app_version || '알 수 없음'}
                  </div>
                </div>
                <div>
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase">
                    내보낸 시각
                  </div>
                  <div className="text-sm font-bold text-foreground mt-1 truncate">
                    {preview.exported_at
                      ? new Date(preview.exported_at).toLocaleDateString()
                      : '알 수 없음'}
                  </div>
                </div>
                <div>
                  <div className="text-[10px] font-semibold text-muted-foreground uppercase">
                    백업 총 크기
                  </div>
                  <div className="text-sm font-bold text-foreground mt-1">
                    {formatBytes(preview.total_size_bytes)}
                  </div>
                </div>
              </div>

              {/* Sections List */}
              <div className="flex flex-col gap-2">
                <span className="text-xs font-semibold text-muted-foreground">
                  백업 포함 섹션 리스트
                </span>
                <div className="rounded-xl border overflow-hidden bg-background/20">
                  <table className="min-w-full divide-y divide-border">
                    <thead className="bg-muted/40">
                      <tr>
                        <th className="px-4 py-2 text-left text-[10px] font-semibold text-muted-foreground uppercase">
                          섹션명
                        </th>
                        <th className="px-4 py-2 text-center text-[10px] font-semibold text-muted-foreground uppercase">
                          항목 수
                        </th>
                        <th className="px-4 py-2 text-right text-[10px] font-semibold text-muted-foreground uppercase">
                          파일 크기
                        </th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border text-xs text-foreground/80">
                      {preview.sections.map((sec) => (
                        <tr key={sec.name} className="hover:bg-background/25">
                          <td className="px-4 py-2 font-medium capitalize">
                            {sec.name.replace('_', ' ')}
                          </td>
                          <td className="px-4 py-2 text-center font-semibold">
                            {sec.item_count}개
                          </td>
                          <td className="px-4 py-2 text-right text-muted-foreground">
                            {formatBytes(sec.size_bytes)}
                          </td>
                        </tr>
                      ))}
                    </tbody>
                  </table>
                </div>
              </div>

              {/* Strategy Selection */}
              <div className="flex flex-col gap-3">
                <span className="text-xs font-semibold text-muted-foreground">
                  가져오기 충돌 해결 방식
                </span>
                <div className="grid grid-cols-3 gap-3">
                  <div
                    onClick={() => setStrategy('skip')}
                    className={`p-3 rounded-xl border cursor-pointer hover:bg-background/40 transition-all duration-200 flex flex-col gap-1.5 ${
                      strategy === 'skip'
                        ? 'border-primary bg-primary/5 ring-1 ring-primary'
                        : 'bg-background/20'
                    }`}
                  >
                    <span className="text-xs font-bold">Skip (기존 유지)</span>
                    <span className="text-[10px] text-muted-foreground leading-relaxed">
                      이름이나 ID가 겹칠 경우 현재 기기의 기존 설정을
                      보존합니다.
                    </span>
                  </div>
                  <div
                    onClick={() => setStrategy('overwrite')}
                    className={`p-3 rounded-xl border cursor-pointer hover:bg-background/40 transition-all duration-200 flex flex-col gap-1.5 ${
                      strategy === 'overwrite'
                        ? 'border-primary bg-primary/5 ring-1 ring-primary'
                        : 'bg-background/20'
                    }`}
                  >
                    <span className="text-xs font-bold text-destructive">
                      Overwrite (덮어쓰기)
                    </span>
                    <span className="text-[10px] text-muted-foreground leading-relaxed font-medium">
                      기존의 로컬 레코드를 전부 제거하고 백업 데이터로 완전
                      교체합니다.
                    </span>
                  </div>
                  <div
                    onClick={() => setStrategy('merge')}
                    className={`p-3 rounded-xl border cursor-pointer hover:bg-background/40 transition-all duration-200 flex flex-col gap-1.5 ${
                      strategy === 'merge'
                        ? 'border-primary bg-primary/5 ring-1 ring-primary'
                        : 'bg-background/20'
                    }`}
                  >
                    <span className="text-xs font-bold">Merge (병합)</span>
                    <span className="text-[10px] text-muted-foreground leading-relaxed">
                      {
                        "기존 설정을 유지하면서 새 설정 항목을 주입합니다. settings 외의 다른 데이터는 안전을 위해 '건너뛰기(Skip)'로 처리됩니다."
                      }
                    </span>
                  </div>
                </div>
              </div>
            </CardContent>

            <CardFooter className="flex justify-between gap-3 border-t pt-4 bg-muted/20">
              <Button
                variant="ghost"
                onClick={reset}
                className="rounded-xl h-10"
              >
                취소 및 뒤로가기
              </Button>
              <Button
                onClick={handleImport}
                disabled={
                  typeof preview.compatibility === 'object' &&
                  'Incompatible' in preview.compatibility
                }
                className="gap-2 rounded-xl h-10 shadow-sm"
              >
                <ArrowDownToLine className="h-4 w-4" />
                가져오기 실행
              </Button>
            </CardFooter>
          </Card>
        )}

        {/* Complete Result Screen */}
        {phase === 'complete' && (exportInfo || importResult) && (
          <Card className="border border-border/80 shadow-lg bg-card/40 backdrop-blur-md rounded-2xl p-8 flex flex-col items-center gap-6 animate-in fade-in duration-300">
            <div className="h-16 w-16 rounded-full bg-primary/10 text-primary flex items-center justify-center animate-bounce">
              <CheckCircle2 className="h-8 w-8" />
            </div>

            <div className="text-center">
              <h2 className="text-xl font-bold text-foreground">
                {exportInfo ? '내보내기 작업 완료!' : '가져오기 작업 성공!'}
              </h2>
              <p className="text-sm text-muted-foreground mt-2 max-w-md">
                {exportInfo
                  ? '현재 환경의 설정 백업 파일이 정상적으로 패키징 및 생성되었습니다.'
                  : '가져온 설정이 무결성 검증을 마치고 안전하게 주입되었습니다. 변화를 활성화하기 위해 아래 정리를 수행해 주십시오.'}
              </p>
            </div>

            <Separator />

            {exportInfo && (
              <div className="w-full flex flex-col gap-2 text-xs p-4 rounded-xl border bg-background/30 text-left">
                <div className="flex justify-between">
                  <span className="text-muted-foreground">저장 경로:</span>
                  <span className="font-mono text-foreground font-semibold truncate max-w-sm">
                    {exportInfo.file_path}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">파일 크기:</span>
                  <span className="text-foreground font-semibold">
                    {formatBytes(exportInfo.file_size_bytes)}
                  </span>
                </div>
                <div className="flex justify-between">
                  <span className="text-muted-foreground">포함 구역:</span>
                  <span className="text-foreground font-semibold">
                    {exportInfo.sections.join(', ')}
                  </span>
                </div>
              </div>
            )}

            {importResult && (
              <div className="w-full flex flex-col gap-4">
                <div className="rounded-xl border overflow-hidden bg-background/20 text-xs text-left">
                  <table className="min-w-full divide-y divide-border">
                    <thead className="bg-muted/40">
                      <tr>
                        <th className="px-4 py-2 font-semibold text-muted-foreground uppercase">
                          섹션
                        </th>
                        <th className="px-4 py-2 text-center font-semibold text-muted-foreground uppercase">
                          성공
                        </th>
                        <th className="px-4 py-2 text-center font-semibold text-muted-foreground uppercase">
                          스킵
                        </th>
                        <th className="px-4 py-2 text-right font-semibold text-muted-foreground uppercase">
                          오류
                        </th>
                      </tr>
                    </thead>
                    <tbody className="divide-y divide-border text-foreground/80">
                      {Object.entries(importResult.sections_imported).map(
                        ([name, report]) => (
                          <tr key={name} className="hover:bg-background/25">
                            <td className="px-4 py-2 font-medium capitalize">
                              {name.replace('_', ' ')}
                            </td>
                            <td className="px-4 py-2 text-center text-primary font-bold">
                              {report.success}
                            </td>
                            <td className="px-4 py-2 text-center text-muted-foreground font-semibold">
                              {report.skipped}
                            </td>
                            <td className="px-4 py-2 text-right text-destructive font-bold">
                              {report.errors.length}
                            </td>
                          </tr>
                        ),
                      )}
                    </tbody>
                  </table>
                </div>

                <div className="p-4 rounded-xl border border-primary/20 bg-primary/5 flex flex-col gap-3 text-left">
                  <div className="flex gap-2.5 items-start text-xs text-muted-foreground leading-relaxed">
                    <FolderSync className="h-5 w-5 text-primary shrink-0" />
                    <div>
                      <span className="font-semibold text-foreground">
                        🔌 MCP 및 백그라운드 환경 복원:
                      </span>{' '}
                      가져오기 이후 MCP 서비스가 새로운 호스트/환경에서 정상
                      가동되기 위해 토큰 갱신 및 검증이 필요합니다. 아래 검증
                      버튼을 클릭하십시오.
                    </div>
                  </div>
                  <Button
                    onClick={handleReverify}
                    disabled={reverifying}
                    className="w-full gap-2 rounded-lg h-9 shadow-sm"
                  >
                    <RefreshCw
                      className={`h-4 w-4 ${reverifying ? 'animate-spin' : ''}`}
                    />
                    MCP 서버 재인증 및 정합성 검증
                  </Button>
                </div>
              </div>
            )}

            <Button
              onClick={reset}
              className="w-full rounded-xl h-10 font-bold"
              variant="outline"
            >
              마이그레이션 메인으로 이동
            </Button>
          </Card>
        )}
      </div>

      {/* Export Password Modal */}
      <Dialog
        open={isExportPasswordOpen}
        onOpenChange={setIsExportPasswordOpen}
      >
        <DialogContent className="max-w-md rounded-2xl bg-card border border-border shadow-lg p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-foreground">
              백업 보안 비밀번호 설정
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground mt-2">
              민감한 데이터(API 키, 자격 증명 등)를 포함하여 백업 파일을
              생성하므로, 백업 파일을 암호화하여 보호하기 위해 비밀번호를
              설정해야 합니다. 이 비밀번호는 나중에 백업을 복원(가져오기)할 때
              사용됩니다.
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4 my-2">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                비밀번호 입력 (최소 4자리)
              </label>
              <Input
                type="password"
                placeholder="비밀번호를 입력하십시오"
                value={exportPassword}
                onChange={(e) => setExportPassword(e.target.value)}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                비밀번호 확인
              </label>
              <Input
                type="password"
                placeholder="비밀번호를 다시 한번 입력하십시오"
                value={exportPasswordConfirm}
                onChange={(e) => setExportPasswordConfirm(e.target.value)}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            {exportPasswordError && (
              <span className="text-xs text-destructive font-semibold">
                ⚠️ {exportPasswordError}
              </span>
            )}
          </div>

          <DialogFooter className="gap-2 flex justify-end">
            <Button
              variant="outline"
              size="sm"
              onClick={() => setIsExportPasswordOpen(false)}
              className="rounded-lg h-9"
            >
              취소
            </Button>
            <Button
              size="sm"
              onClick={handleExportWithPassword}
              className="rounded-lg h-9 font-semibold"
            >
              내보내기 실행
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>

      {/* Import Password Modal */}
      <Dialog
        open={isImportPasswordOpen}
        onOpenChange={(open) => {
          if (!open) handleCancelImportPassword();
        }}
      >
        <DialogContent className="max-w-md rounded-2xl bg-card border border-border shadow-lg p-6">
          <DialogHeader>
            <DialogTitle className="text-lg font-bold text-foreground">
              보안 비밀번호 입력
            </DialogTitle>
            <DialogDescription className="text-xs text-muted-foreground mt-2">
              이 백업 파일은 암호화되어 있어 미리보기와 가져오기를 수행하려면
              비밀번호가 필요합니다. 설정하신 비밀번호를 입력해주세요.
            </DialogDescription>
          </DialogHeader>

          <div className="flex flex-col gap-4 my-2">
            <div className="flex flex-col gap-1.5">
              <label className="text-xs font-semibold text-muted-foreground">
                비밀번호 입력
              </label>
              <Input
                type="password"
                placeholder="비밀번호를 입력하십시오"
                value={importPassword}
                onChange={(e) => setImportPassword(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === 'Enter') handleImportPasswordSubmit();
                }}
                className="h-9 rounded-lg border bg-background/50 text-sm focus:ring-1 focus:ring-primary"
              />
            </div>

            {importPasswordError && (
              <span className="text-xs text-destructive font-semibold">
                ⚠️ {importPasswordError}
              </span>
            )}
          </div>

          <DialogFooter className="gap-2 flex justify-end">
            <Button
              variant="outline"
              size="sm"
              onClick={handleCancelImportPassword}
              className="rounded-lg h-9"
            >
              취소
            </Button>
            <Button
              size="sm"
              onClick={handleImportPasswordSubmit}
              className="rounded-lg h-9 font-semibold"
            >
              확인
            </Button>
          </DialogFooter>
        </DialogContent>
      </Dialog>
    </div>
  );
};

export default MigrationPage;
