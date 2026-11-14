# SPDX-License-Identifier: GPL-2.0-or-later
"""Shared launcher's platform notices; Russian is supplied at each call site."""
import bbport_lang

KEYS = [
    'The automated route benchmark currently requires Windows.',
    'This provider currently requires Windows.',
    'Windows update packages cannot be installed on this platform.',
    'In Steam, add the Linux launcher as a non-Steam game and set launch options to --play.',
    'BBSC DLL injection currently requires Windows. Regular online play is available on the Online play page.',
    'On Linux, update the source checkout and rebuild the port. Windows update packages are not installed here.',
    'Shortcut created: {}',
]
TEXTS = {
    'ar': [
        'اختبار المسار الآلي يتطلب Windows حاليًا.',
        'هذا المزوّد يتطلب Windows حاليًا.',
        'لا يمكن تثبيت حزم تحديث Windows على هذه المنصة.',
        'في Steam، أضف مشغّل Linux كلعبة من خارج Steam واضبط خيارات التشغيل على --play.',
        'حقن مكتبة BBSC يتطلب Windows حاليًا. اللعب العادي عبر الإنترنت متاح في صفحة اللعب عبر الإنترنت.',
        'على Linux، حدّث الشفرة المصدرية وأعد بناء المنفذ. لا تُثبّت حزم تحديث Windows هنا.',
        'تم إنشاء الاختصار: {}',
    ],
    'es': [
        'La prueba automática de recorrido requiere Windows por ahora.',
        'Este proveedor requiere Windows por ahora.',
        'No se pueden instalar paquetes de actualización de Windows en esta plataforma.',
        'En Steam, añade el lanzador de Linux como juego externo y establece --play como opción de lanzamiento.',
        'La inyección de la DLL de BBSC requiere Windows por ahora. El juego en línea normal está en la página Juego en línea.',
        'En Linux, actualiza el código fuente y vuelve a compilar el port. Aquí no se instalan actualizaciones de Windows.',
        'Acceso directo creado: {}',
    ],
    'pt': [
        'O teste automático de percurso requer Windows no momento.',
        'Este provedor requer Windows no momento.',
        'Não é possível instalar pacotes de atualização do Windows nesta plataforma.',
        'No Steam, adicione o inicializador Linux como jogo não Steam e defina --play nas opções de inicialização.',
        'A injeção da DLL do BBSC requer Windows no momento. O modo online normal está na página Jogar online.',
        'No Linux, atualize o código-fonte e recompile o port. Pacotes de atualização do Windows não são instalados aqui.',
        'Atalho criado: {}',
    ],
    'fr': [
        'Le test de parcours automatique nécessite actuellement Windows.',
        'Ce fournisseur nécessite actuellement Windows.',
        'Les mises à jour Windows ne peuvent pas être installées sur cette plateforme.',
        'Dans Steam, ajoutez le lanceur Linux comme jeu non Steam et définissez --play comme option de lancement.',
        'L’injection de la DLL BBSC nécessite actuellement Windows. Le jeu en ligne normal est disponible sur la page Jeu en ligne.',
        'Sous Linux, actualisez les sources et recompilez le port. Les mises à jour Windows ne sont pas installées ici.',
        'Raccourci créé : {}',
    ],
    'de': [
        'Der automatische Streckenbenchmark erfordert derzeit Windows.',
        'Dieser Anbieter erfordert derzeit Windows.',
        'Windows-Updatepakete können auf dieser Plattform nicht installiert werden.',
        'Füge den Linux-Launcher in Steam als Steam-fremdes Spiel hinzu und setze die Startoptionen auf --play.',
        'Die BBSC-DLL-Injektion erfordert derzeit Windows. Reguläres Onlinespiel ist auf der Seite Onlinespiel verfügbar.',
        'Aktualisiere unter Linux den Quellcode und kompiliere den Port neu. Windows-Updatepakete werden hier nicht installiert.',
        'Verknüpfung erstellt: {}',
    ],
    'it': [
        'Il benchmark automatico del percorso richiede attualmente Windows.',
        'Questo provider richiede attualmente Windows.',
        'I pacchetti di aggiornamento Windows non possono essere installati su questa piattaforma.',
        'In Steam, aggiungi il launcher Linux come gioco non di Steam e imposta --play nelle opzioni di avvio.',
        'L’iniezione della DLL BBSC richiede attualmente Windows. Il normale gioco online è disponibile nella pagina Gioco online.',
        'Su Linux, aggiorna i sorgenti e ricompila il port. Qui non vengono installati aggiornamenti Windows.',
        'Collegamento creato: {}',
    ],
    'pl': [
        'Automatyczny test trasy wymaga obecnie systemu Windows.',
        'Ten dostawca wymaga obecnie systemu Windows.',
        'Nie można instalować pakietów aktualizacji Windows na tej platformie.',
        'Dodaj launcher Linux do Steam jako grę spoza Steam i ustaw opcję uruchamiania --play.',
        'Wstrzykiwanie biblioteki DLL BBSC wymaga obecnie Windows. Zwykła gra online jest dostępna na stronie Gra online.',
        'W systemie Linux zaktualizuj kod źródłowy i ponownie skompiluj port. Pakiety aktualizacji Windows nie są tu instalowane.',
        'Utworzono skrót: {}',
    ],
    'tr': [
        'Otomatik rota testi şu anda Windows gerektiriyor.',
        'Bu sağlayıcı şu anda Windows gerektiriyor.',
        'Windows güncelleme paketleri bu platforma yüklenemez.',
        'Steam’de Linux başlatıcısını Steam dışı oyun olarak ekleyin ve başlatma seçeneklerini --play olarak ayarlayın.',
        'BBSC DLL enjeksiyonu şu anda Windows gerektiriyor. Normal çevrimiçi oyun, Çevrimiçi oyun sayfasında kullanılabilir.',
        'Linux’ta kaynak kodunu güncelleyin ve portu yeniden derleyin. Windows güncelleme paketleri burada yüklenmez.',
        'Kısayol oluşturuldu: {}',
    ],
    'zh': [
        '自动路线基准测试目前需要 Windows。',
        '此提供程序目前需要 Windows。',
        '无法在此平台上安装 Windows 更新包。',
        '在 Steam 中将 Linux 启动器添加为非 Steam 游戏，并将启动选项设为 --play。',
        'BBSC DLL 注入目前需要 Windows。普通在线游玩可在“在线游玩”页面使用。',
        '在 Linux 上，请更新源代码并重新构建移植版。此处不会安装 Windows 更新包。',
        '已创建快捷方式：{}',
    ],
    'ja': [
        '自動ルートベンチマークには現在 Windows が必要です。',
        'このプロバイダーには現在 Windows が必要です。',
        'このプラットフォームには Windows 用更新パッケージをインストールできません。',
        'Steam で Linux ランチャーを非 Steam ゲームとして追加し、起動オプションを --play に設定してください。',
        'BBSC の DLL インジェクションには現在 Windows が必要です。通常のオンラインプレイはオンラインプレイページで利用できます。',
        'Linux ではソースコードを更新して移植版を再ビルドしてください。ここでは Windows 用更新パッケージはインストールされません。',
        'ショートカットを作成しました：{}',
    ],
    'ko': [
        '자동 경로 벤치마크에는 현재 Windows가 필요합니다.',
        '이 공급자에는 현재 Windows가 필요합니다.',
        '이 플랫폼에는 Windows 업데이트 패키지를 설치할 수 없습니다.',
        'Steam에서 Linux 런처를 비 Steam 게임으로 추가하고 시작 옵션을 --play로 설정하세요.',
        'BBSC DLL 주입에는 현재 Windows가 필요합니다. 일반 온라인 플레이는 온라인 플레이 페이지에서 이용할 수 있습니다.',
        'Linux에서는 소스 코드를 업데이트하고 포트를 다시 빌드하세요. 여기서는 Windows 업데이트 패키지를 설치하지 않습니다.',
        '바로 가기 생성됨: {}',
    ],
}

for language, texts in TEXTS.items():
    assert len(texts) == len(KEYS), language
    bbport_lang.EXTRA_TRANSLATIONS.setdefault(language, {}).update(zip(KEYS, texts))
