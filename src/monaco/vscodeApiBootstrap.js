import { initialize } from '@codingame/monaco-vscode-api'
import { registerExtension } from '@codingame/monaco-vscode-api/extensions'
import getConfigurationServiceOverride, {
  initUserConfiguration,
} from '@codingame/monaco-vscode-configuration-service-override'
import getLanguagesServiceOverride from '@codingame/monaco-vscode-languages-service-override'
import getLogServiceOverride from '@codingame/monaco-vscode-log-service-override'
import getModelServiceOverride from '@codingame/monaco-vscode-model-service-override'
import getTextmateServiceOverride from '@codingame/monaco-vscode-textmate-service-override'
import getThemeServiceOverride from '@codingame/monaco-vscode-theme-service-override'
import { createLogger } from '../utils/logger'
import { ensureVsCodeVirtualFileSystemProvidersRegistered } from './vscodeModelBridge'

const logger = createLogger('vscodeApiBootstrap')

let vscodeApiInitPromise = null

const GUAXI_VSCODE_USER_CONFIGURATION = {
  'editor.semanticHighlighting.enabled': true,
  'editor.wordBasedSuggestions': 'off',
  'typescript.preferences.includePackageJsonAutoImports': 'auto',
  'javascript.preferences.includePackageJsonAutoImports': 'auto',
  'python.analysis.autoImportCompletions': true,
  'python.analysis.typeCheckingMode': 'basic',
}

export const initializeGuaxiVscodeServices = async () => {
  if (vscodeApiInitPromise) return vscodeApiInitPromise

  vscodeApiInitPromise = (async () => {
    ensureVsCodeVirtualFileSystemProvidersRegistered()
    await initUserConfiguration(JSON.stringify(GUAXI_VSCODE_USER_CONFIGURATION))

    // Design note:
    // 1. We intentionally initialize only the services needed by
    //    monaco-languageclient: configuration/log/model/languages plus
    //    TextMate/theme for VS Code level tokenization.
    // 2. MonacoVscodeApiWrapper's EditorService path imports Workbench Views
    //    modules, and those modules register MenuId('Views') at module scope.
    //    In this app that module can already be present through other
    //    @codingame packages, so the second registration throws at startup.
    // 3. Keeping Views/Workbench out of the production LSP bootstrap gives us
    //    the standard language client providers without pulling in duplicate
    //    workbench menu registries.
    await initialize({
      ...getConfigurationServiceOverride(),
      ...getLogServiceOverride(),
      ...getModelServiceOverride(),
      ...getLanguagesServiceOverride(),
      ...getTextmateServiceOverride(),
      ...getThemeServiceOverride(),
    })

    logger.info('[VSCodeApi] monaco-vscode language services initialized')

    // 注册 GUAXI 任务块语言（.org）到 vscode-api 语言服务，否则自定义语言会回退为 Plain Text，
    // 导致 Monarch/语义高亮与补全 provider 都不生效。
    try {
      registerExtension({
        name: 'guaxi-task',
        publisher: 'guaxi',
        version: '1.0.0',
        engines: { vscode: '*' },
        contributes: {
          languages: [
            {
              id: 'guaxitask',
              extensions: ['.org'],
              aliases: ['GuaxiTask', 'guaxitask', 'org'],
            },
          ],
        },
      })
      logger.info('[VSCodeApi] guaxitask language registered')
    } catch (e) {
      logger.warn('[VSCodeApi] register guaxitask language failed', e)
    }

    return true
  })().catch((error) => {
    vscodeApiInitPromise = null
    logger.error('[VSCodeApi] monaco-vscode language services init failed', { error })
    throw error
  })

  return vscodeApiInitPromise
}
