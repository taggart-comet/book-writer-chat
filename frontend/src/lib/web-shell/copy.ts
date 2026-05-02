export type UiLanguage = 'en' | 'ru';

export type AppCopy = {
  appName: string;
  switchToEnglish: string;
  switchToRussian: string;
  webAuthentication: string;
  restoringSession: string;
  restoringSessionCopy: string;
  signInToMessenger: string;
  deploymentUsesOperator: string;
  username: string;
  password: string;
  signingIn: string;
  signIn: string;
  invalidUsernameOrPassword: string;
  loginUnavailable: string;
  sessionExpired: string;
  unableToRestoreSession: string;
  books: string;
  edits: string;
  pravki: string;
  createBook: string;
  provisionWorkspace: string;
  bookTitle: string;
  language: string;
  creating: string;
  createBookAction: string;
  availableBooks: string;
  booksLandingEyebrow: string;
  booksLandingTitle: string;
  booksLandingCopy: string;
  loadingBooks: string;
  noBooks: string;
  updatedAt: (value: string) => string;
  openBook: string;
  previewBook: string;
  bookHomeEyebrow: string;
  bookHomeTitle: string;
  readerShortcutCopy: string;
  editsShortcutCopy: string;
  backToBooks: string;
  backToBook: string;
  backToEdits: string;
  goToPreview: string;
  editsEyebrow: string;
  editsTitle: string;
  editsCopy: string;
  conversationList: string;
  newConversation: string;
  loadingConversations: string;
  noConversations: string;
  latestActive: string;
  lastActivity: (value: string) => string;
  chatEyebrow: string;
  chatTitle: string;
  chatCopy: string;
  loadingTranscript: string;
  transcriptEmpty: string;
  failedToLoadBooks: string;
  failedToCreateBook: string;
  failedToLoadConversations: string;
  failedToCreateConversation: string;
  failedToLoadTranscript: string;
  failedToSendMessage: string;
  conversationNotFound: string;
  pendingReaderMention: string;
  mentionNewConversation: string;
  mentionLatestConversation: string;
  dismiss: string;
  timestampUnavailable: string;
  you: string;
  sendMessage: string;
  messagePlaceholder: string;
  sendingMessage: string;
  workInProgress: string;
  messageRequired: string;
  languageLabels: {
    en: string;
    ru: string;
  };
};

export const copy: Record<UiLanguage, AppCopy> = {
  en: {
    appName: 'Book Writer Chat',
    switchToEnglish: 'English',
    switchToRussian: 'Русский',
    webAuthentication: 'Web authentication',
    restoringSession: 'Restoring session',
    restoringSessionCopy: 'Checking the stored JWT before opening the workspace.',
    signInToMessenger: 'Sign in to the workspace',
    deploymentUsesOperator:
      'This deployment uses one operator account configured by environment variables.',
    username: 'Username',
    password: 'Password',
    signingIn: 'Signing in…',
    signIn: 'Sign in',
    invalidUsernameOrPassword: 'Invalid username or password.',
    loginUnavailable: 'Login is unavailable right now.',
    sessionExpired: 'Your session expired. Sign in again.',
    unableToRestoreSession: 'Unable to restore the authenticated session.',
    books: 'Books',
    edits: 'Edits',
    pravki: 'Pravki',
    createBook: 'Create book',
    provisionWorkspace: 'Provision a new workspace',
    bookTitle: 'Book title',
    language: 'Language',
    creating: 'Creating…',
    createBookAction: 'Create book',
    availableBooks: 'Available books',
    booksLandingEyebrow: 'Book workspaces',
    booksLandingTitle: 'Choose a book or create a new one',
    booksLandingCopy: 'The main screen is focused only on your books and workspace creation.',
    loadingBooks: 'Loading books…',
    noBooks: 'No books exist yet. Create one to start the workflow.',
    updatedAt: (value: string) => `Updated ${value}`,
    openBook: 'Open book',
    previewBook: 'Book preview',
    bookHomeEyebrow: 'Book workspace',
    bookHomeTitle: 'Choose what to open next',
    readerShortcutCopy: 'Open the latest rendered draft in the reader.',
    editsShortcutCopy: 'Review conversations and continue editing requests.',
    backToBooks: 'Back to books',
    backToBook: 'Back to book',
    backToEdits: 'Back to edits',
    goToPreview: 'Go to preview',
    editsEyebrow: 'Edits',
    editsTitle: 'Conversations for this book',
    editsCopy: 'Open an existing conversation or create a new one.',
    conversationList: 'Conversation list',
    newConversation: 'New conversation',
    loadingConversations: 'Loading conversations…',
    noConversations: 'No conversations exist for this book yet.',
    latestActive: 'Latest active',
    lastActivity: (value: string) => `Last activity ${value}`,
    chatEyebrow: 'Conversation',
    chatTitle: 'Conversation workspace',
    chatCopy: 'Write the next request here or jump straight to the reader.',
    loadingTranscript: 'Loading transcript…',
    transcriptEmpty: 'This conversation does not have user-visible messages yet.',
    failedToLoadBooks: 'Failed to load books.',
    failedToCreateBook: 'Failed to create the book.',
    failedToLoadConversations: 'Failed to load conversations.',
    failedToCreateConversation: 'Failed to create the conversation.',
    failedToLoadTranscript: 'Failed to load the transcript.',
    failedToSendMessage: 'Failed to send the message.',
    conversationNotFound: 'This conversation could not be found for the selected book.',
    pendingReaderMention: 'Pending reader mention',
    mentionNewConversation: 'Created from a new conversation handoff.',
    mentionLatestConversation: 'Attached to the latest active conversation.',
    dismiss: 'Dismiss',
    timestampUnavailable: 'Timestamp unavailable',
    you: 'You',
    sendMessage: 'Send message',
    messagePlaceholder: 'Describe what Codex should do next.',
    sendingMessage: 'Sending…',
    workInProgress: 'Work on your request is in progress.',
    messageRequired: 'Enter a message before sending it.',
    languageLabels: {
      en: 'English',
      ru: 'Russian'
    }
  },
  ru: {
    appName: 'Book Writer Chat',
    switchToEnglish: 'English',
    switchToRussian: 'Русский',
    webAuthentication: 'Веб-аутентификация',
    restoringSession: 'Восстанавливаем сессию',
    restoringSessionCopy: 'Подождите немножко',
    signInToMessenger: 'Вход',
    deploymentUsesOperator:
      'В этом развертывании используется одна операторская учетная запись, заданная через переменные окружения.',
    username: 'Имя пользователя',
    password: 'Пароль',
    signingIn: 'Входим…',
    signIn: 'Войти',
    invalidUsernameOrPassword: 'Неверное имя пользователя или пароль.',
    loginUnavailable: 'Сейчас вход недоступен.',
    sessionExpired: 'Сессия истекла. Войдите снова.',
    unableToRestoreSession: 'Не удалось восстановить авторизованную сессию.',
    books: 'Книги',
    edits: 'Правки',
    pravki: 'Правки',
    createBook: 'Создать книгу',
    provisionWorkspace: 'Создать новое рабочее пространство',
    bookTitle: 'Название книги',
    language: 'Язык',
    creating: 'Создаем…',
    createBookAction: 'Создать книгу',
    availableBooks: 'Доступные книги',
    booksLandingEyebrow: 'Рабочие пространства книг',
    booksLandingTitle: 'Выберите книгу или создайте новую',
    booksLandingCopy:
      'На главном экране остаются только список книг и создание нового рабочего пространства.',
    loadingBooks: 'Загружаем книги…',
    noBooks: 'Книг пока нет. Создайте первую, чтобы начать работу.',
    updatedAt: (value: string) => `Обновлено ${value}`,
    openBook: 'Открыть книгу',
    previewBook: 'Просмотр книги',
    bookHomeEyebrow: 'Рабочее пространство книги',
    bookHomeTitle: 'Выберите следующий экран',
    readerShortcutCopy: 'Открыть последнюю версию книги.',
    editsShortcutCopy: 'Просмотреть разговоры и продолжить поток правок.',
    backToBooks: 'К книгам',
    backToBook: 'Назад',
    backToEdits: 'К правкам',
    goToPreview: 'Открыть просмотр',
    editsEyebrow: 'Правки',
    editsTitle: 'Правки по этой книге',
    editsCopy: 'Откройте существующий разговор или создайте новый.',
    conversationList: 'Список разговоров',
    newConversation: 'Начать новые правки',
    loadingConversations: 'Загружаем разговоры…',
    noConversations: 'Для этой книги пока нет разговоров.',
    latestActive: 'Последний активный',
    lastActivity: (value: string) => `Последняя активность ${value}`,
    chatEyebrow: 'Разговор',
    chatTitle: 'Рабочее пространство разговора',
    chatCopy: 'Напишите следующий запрос здесь или сразу перейдите в читалку.',
    loadingTranscript: 'Загружаем расшифровку…',
    transcriptEmpty: 'В этом разговоре пока нет сообщений, видимых пользователю.',
    failedToLoadBooks: 'Не удалось загрузить книги.',
    failedToCreateBook: 'Не удалось создать книгу.',
    failedToLoadConversations: 'Не удалось загрузить разговоры.',
    failedToCreateConversation: 'Не удалось создать разговор.',
    failedToLoadTranscript: 'Не удалось загрузить расшифровку.',
    failedToSendMessage: 'Не удалось отправить сообщение.',
    conversationNotFound: 'Не удалось найти этот разговор для выбранной книги.',
    pendingReaderMention: 'Ожидающее упоминание из читалки',
    mentionNewConversation: 'Создано при передаче в новый разговор.',
    mentionLatestConversation: 'Прикреплено к последнему активному разговору.',
    dismiss: 'Закрыть',
    timestampUnavailable: 'Время недоступно',
    you: 'Вы',
    sendMessage: 'Отправить сообщение',
    messagePlaceholder: 'Опишите, что нужно сделать дальше.',
    sendingMessage: 'Отправляем…',
    workInProgress: 'Работа по вашему запросу уже идет.',
    messageRequired: 'Сначала введите сообщение.',
    languageLabels: {
      en: 'Английский',
      ru: 'Русский'
    }
  }
};
