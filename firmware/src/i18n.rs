// ============================================================================= //
// File          : i18n.rs                                                       //
// License       : MIT                                                           //
// Created       : 2026-10-04                                                    //
// Last modified : 2026-10-04                                                    //
// Author        : Daniel Kharlamov                                              //
// Last editor   : Daniel Kharlamov <daniel.kharlamov@googlemail.com>            //
// ============================================================================= //
//                                                                               //
// Summary:                                                                      //
// --------                                                                      //
// Language selection and translations for the device interface.                 //
//                                                                               //
// Module:                                                                       //
// -------                                                                       //
// Defines supported languages with stable indices for saved preferences and API //
// language codes. Translates interface strings and multiline prompts, with      //
// fallback to the supplied text. Includes a test protecting persisted language  //
// indices.                                                                      //
// ============================================================================= //

//! Language selection and translations for the device interface.

use serde::{Deserialize, Serialize};
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Language {
    #[serde(alias = "Turkish")]
    // Represent english as a distinct selectable state or action; the matching handler determines
    // its effect.
    English,
    // Represent spanish as a distinct selectable state or action; the matching handler determines
    // its effect.
    Spanish,
    // Represent german as a distinct selectable state or action; the matching handler determines
    // its effect.
    German,
    // Represent french as a distinct selectable state or action; the matching handler determines
    // its effect.
    French,
    // Represent ukrainian as a distinct selectable state or action; the matching handler determines
    // its effect.
    Ukrainian,
    // Represent swedish as a distinct selectable state or action; the matching handler determines
    // its effect.
    Swedish,
    // Represent italian as a distinct selectable state or action; the matching handler determines
    // its effect.
    Italian,
    // Represent russian as a distinct selectable state or action; the matching handler determines
    // its effect.
    Russian,
}
impl Language {
    // Keep language order stable because saved preference indices depend on these exact positions.
    pub const ALL: [Self; 8] = [
        Self::English,
        Self::Spanish,
        Self::German,
        Self::French,
        Self::Ukrainian,
        Self::Swedish,
        Self::Italian,
        Self::Russian,
    ];
    /// Looks up a supported language by its persisted index.
    ///
    /// # Arguments
    ///
    /// * `index` (`u32`) - Persisted language index to look up.
    ///
    /// # Returns
    ///
    /// `Option<Self>` - Some language for an existing slot; None for an out-of-range index.
    pub fn from_index(index: u32) -> Option<Self> {
        // Execute copied for get result for the surrounding operation.
        Self::ALL.get(index as usize).copied()
    }
    /// Returns the language's stable persisted position.
    ///
    /// # Arguments
    ///
    /// * `self` (`Language`) - Receiver state used by this operation. Passed by value.
    ///
    /// # Returns
    ///
    /// `usize` - Zero-based index into the supported language list.
    pub fn index(self) -> usize {
        self as usize
    }
    /// Returns the language's native display name.
    ///
    /// # Arguments
    ///
    /// * `self` (`Language`) - Receiver state used by this operation. Passed by value.
    ///
    /// # Returns
    ///
    /// `&'static str` - Static localized language name for the settings selector.
    pub fn name(self) -> &'static str {
        [
            "English",
            "Español",
            "Deutsch",
            "Français",
            "Українська",
            "Svenska",
            "Italiano",
            "Русский",
        ][self.index()]
    }
    /// Returns the language code used in location requests.
    ///
    /// # Arguments
    ///
    /// * `self` (`Language`) - Receiver state used by this operation. Passed by value.
    ///
    /// # Returns
    ///
    /// `&'static str` - Static API language code corresponding to this language.
    pub fn code(self) -> &'static str {
        ["en", "es", "de", "fr", "uk", "sv", "it", "ru"][self.index()]
    }
}
/// Translates a known interface string into the selected language.
///
/// # Arguments
///
/// * `language` (`Language`) - Supported language used for translated labels or API
///   requests.
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
///
/// # Returns
///
/// `&str` - Borrowed translation, or the original text when no translation is defined.
pub fn tr(language: Language, text: &str) -> &str {
    // Each arm maps an English lookup key to eight translations in Language::ALL order. The stable
    // language index selects the corresponding text; the fallback returns unknown keys unchanged.
    // Choose the appropriate path for sanitized or clipped text prepared for the available drawing
    // area; each arm handles one supported case.
    match text {
        "min" => ["min", "min", "min", "min", "хв", "min", "min", "мин"][language.index()],
        "Clock" => ["Clock", "Reloj", "Uhr", "Horloge", "Годинник", "Klocka", "Orologio", "Часы"][language.index()],
        "Week" => ["Week", "Semana", "Woche", "Semaine", "Тиждень", "Vecka", "Settimana", "Неделя"][language.index()],
        "Next 7 hours" => ["Next 7 hours", "Próximas 7 horas", "Nächste 7 Std.", "Prochaines 7 h", "Наступні 7 годин", "Nästa 7 timmar", "Prossime 7 ore", "След. 7 часов"][language.index()],
        "Feels like" => ["Feels like", "Sensación", "Gefühlt", "Ressenti", "Відчутна", "Känns som", "Percepita", "Ощущается"][language.index()],
        "Min" => ["Min", "Mín", "Min", "Min", "Мін", "Min", "Min", "Мин"][language.index()],
        "Max" => ["Max", "Máx", "Max", "Max", "Макс", "Max", "Max", "Макс"][language.index()],
        "Waiting for local time..." => ["Waiting for local time...", "Esperando hora local...", "Warte auf Ortszeit...", "Attente de l’heure locale...", "Очікування місцевого часу...", "Väntar på lokal tid...", "Attendo ora locale...", "Ожидание местного времени..."][language.index()],
        "Weather outdated" => ["Weather outdated", "Datos antiguos", "Wetterdaten veraltet", "Données anciennes", "Застарілі дані погоди", "Väderdata är gamla", "Dati meteo vecchi", "Данные устарели"][language.index()],
        "Weather unavailable. Retrying..." => ["Weather unavailable. Retrying...", "Sin datos. Reintentando...", "Kein Wetter. Neuer Versuch...", "Météo indisponible. Nouvel essai...", "Погода недоступна. Повтор...", "Väder saknas. Försöker igen...", "Meteo assente. Riprovo...", "Нет данных. Повтор..."][language.index()],
        "Loading weather..." => ["Loading weather...", "Cargando el tiempo...", "Lade Wetterdaten...", "Chargement météo...", "Завантаження погоди...", "Hämtar väder...", "Carico il meteo...", "Загрузка погоды..."][language.index()],
        "Tap to change page" => ["Tap to change page", "Toca para cambiar página", "Tippen zum Seitenwechsel", "Toucher pour changer de page", "Торкніться для зміни сторінки", "Tryck för att byta sida", "Tocca per cambiare pagina", "Нажмите для смены экрана"][language.index()],
        "Screen off after" => ["Screen off after", "Apagar tras", "Bildschirm aus", "Écran éteint après", "Вимкнути екран через", "Släck skärmen efter", "Spegni schermo dopo", "Выключать через"][language.index()],
        "Never" => ["Never", "Nunca", "Nie", "Jamais", "Ніколи", "Aldrig", "Mai", "Никогда"][language.index()],
        "Screen brightness update failed. Retrying." => ["Screen brightness update failed. Retrying.", "Falló el brillo. Reintentando.", "Helligkeit fehlgeschlagen. Neuer Versuch.", "Échec de luminosité. Nouvel essai.", "Не вдалося змінити яскравість. Повтор.", "Ljusstyrkan misslyckades. Försöker igen.", "Luminosità fallita. Riprovo.", "Не удалось изменить яркость. Повтор..."][language.index()],
        "Settings" => ["Settings", "Ajustes", "Einstellungen", "Réglages", "Налаштування", "Inställningar", "Impostazioni", "Настройки"][language.index()],
        "Brightness" => ["Brightness", "Brillo", "Helligkeit", "Luminosité", "Яскравість", "Ljusstyrka", "Luminosità", "Яркость"][language.index()],
        "Wake on presence" => ["Wake on presence", "Activar por presencia", "Bei Anwesenheit aufwecken", "Réveil par présence", "Пробудження за присутності", "Väck vid närvaro", "Risveglio con presenza", "Включать при присутствии"][language.index()],
        "Night mode" => ["Night mode", "Modo nocturno", "Nachtmodus", "Mode nuit", "Нічний режим", "Nattläge", "Modalità notte", "Ночной режим"][language.index()],
        "Fahrenheit" => ["Fahrenheit", "Fahrenheit", "Fahrenheit", "Fahrenheit", "Фаренгейт", "Fahrenheit", "Fahrenheit", "Фаренгейт"][language.index()],
        "24-hour time" => ["24-hour time", "Hora de 24 horas", "24-Stunden-Zeit", "Format 24 heures", "24-годинний час", "24-timmarsformat", "Formato 24 ore", "24-часовой формат"][language.index()],
        "Location" => ["Location", "Ubicación", "Ort", "Lieu", "Місце", "Plats", "Località", "Место"][language.index()],
        "Language" => ["Language", "Idioma", "Sprache", "Langue", "Мова", "Språk", "Lingua", "Язык"][language.index()],
        "Change location" => ["Change location", "Cambiar ubicación", "Ort ändern", "Changer de lieu", "Змінити місце", "Ändra plats", "Cambia località", "Изменить место"][language.index()],
        "Reset Wi-Fi" => ["Reset Wi-Fi", "Restablecer Wi-Fi", "WLAN zurücksetzen", "Réinitialiser Wi-Fi", "Скинути Wi-Fi", "Återställ Wi-Fi", "Reimposta Wi-Fi", "Сброс Wi-Fi"][language.index()],
        "Screen calibration" => ["Screen calibration", "Calibrar pantalla", "Bildschirm kalibrieren", "Calibrer l’écran", "Калібрування екрана", "Kalibrera skärmen", "Calibra schermo", "Калибровка экрана"][language.index()],
        "Close" => ["Close", "Cerrar", "Schließen", "Fermer", "Закрити", "Stäng", "Chiudi", "Закрыть"][language.index()],
        "Back" => ["Back", "Atrás", "Zurück", "Retour", "Назад", "Tillbaka", "Indietro", "Назад"][language.index()],
        "Cancel" => ["Cancel", "Cancelar", "Abbrechen", "Annuler", "Скасувати", "Avbryt", "Annulla", "Отмена"][language.index()],
        "Save" => ["Save", "Guardar", "Speichern", "Enregistrer", "Зберегти", "Spara", "Salva", "Сохранить"][language.index()],
        "Start" => ["Start", "Iniciar", "Starten", "Démarrer", "Почати", "Starta", "Avvia", "Начать"][language.index()],
        "Retry" => ["Retry", "Reintentar", "Erneut versuchen", "Réessayer", "Повторити", "Försök igen", "Riprova", "Повторить"][language.index()],
        "Search" => ["Search", "Buscar", "Suchen", "Rechercher", "Пошук", "Sök", "Cerca", "Поиск"][language.index()],
        "Search results" => ["Search results", "Resultados", "Suchergebnisse", "Résultats", "Результати пошуку", "Sökresultat", "Risultati", "Результаты поиска"][language.index()],
        "Choose a language" => ["Choose a language", "Elegir idioma", "Sprache auswählen", "Choisir une langue", "Виберіть мову", "Välj språk", "Scegli lingua", "Выберите язык"][language.index()],
        "Select a location." => ["Select a location.", "Selecciona una ubicación.", "Wähle einen Ort.", "Choisissez un lieu.", "Виберіть місце.", "Välj en plats.", "Seleziona una località.", "Выберите место."][language.index()],
        "Enter a city or postal code." => ["Enter a city or postal code.", "Introduce ciudad o código postal.", "Stadt oder Postleitzahl eingeben.", "Saisissez une ville ou un code postal.", "Введіть місто або поштовий індекс.", "Ange ort eller postnummer.", "Inserisci città o codice postale.", "Введите город или почтовый индекс."][language.index()],
        "Use at least two characters." => ["Use at least two characters.", "Usa al menos dos caracteres.", "Mindestens zwei Zeichen eingeben.", "Utilisez au moins deux caractères.", "Введіть щонайменше два символи.", "Använd minst två tecken.", "Usa almeno due caratteri.", "Введите не менее двух символов."][language.index()],
        "Searching..." => ["Searching...", "Buscando...", "Suche läuft...", "Recherche...", "Пошук...", "Söker...", "Ricerca...", "Поиск..."][language.index()],
        "No locations found." => ["No locations found.", "No se encontraron ubicaciones.", "Keine Orte gefunden.", "Aucun lieu trouvé.", "Місць не знайдено.", "Inga platser hittades.", "Nessuna località trovata.", "Ничего не найдено."][language.index()],
        "Location search failed. Please retry." => ["Location search failed. Please retry.", "Falló la búsqueda. Reintenta.", "Ortssuche fehlgeschlagen. Erneut versuchen.", "Échec de la recherche. Réessayez.", "Помилка пошуку. Спробуйте знову.", "Platssökningen misslyckades. Försök igen.", "Ricerca fallita. Riprova.", "Ошибка поиска. Повторите."][language.index()],
        "Connect to Wi-Fi to search." => ["Connect to Wi-Fi to search.", "Conecta al Wi-Fi para buscar.", "Zum Suchen mit WLAN verbinden.", "Connectez le Wi-Fi pour rechercher.", "Для пошуку підключіть Wi-Fi.", "Anslut Wi-Fi för att söka.", "Connetti il Wi-Fi per cercare.", "Для поиска подключитесь к Wi-Fi."][language.index()],
        "Waiting for time synchronization." => ["Waiting for time synchronization.", "Esperando sincronización de hora.", "Warte auf Zeitsynchronisierung.", "En attente de synchronisation.", "Очікування синхронізації часу.", "Väntar på tidssynkronisering.", "Attesa della sincronizzazione.", "Ожидание синхронизации времени."][language.index()],
        "Settings could not be read." => ["Settings could not be read.", "No se pudieron leer los ajustes.", "Einstellungen konnten nicht gelesen werden.", "Lecture des réglages impossible.", "Не вдалося прочитати налаштування.", "Inställningarna kunde inte läsas.", "Impossibile leggere le impostazioni.", "Не удалось прочитать настройки."][language.index()],
        "Settings storage failed." => ["Settings storage failed.", "No se guardaron los ajustes.", "Einstellungen konnten nicht gespeichert werden.", "Enregistrement des réglages impossible.", "Не вдалося зберегти налаштування.", "Inställningarna kunde inte sparas.", "Impossibile salvare le impostazioni.", "Не удалось сохранить настройки."][language.index()],
        "Forget the saved Wi-Fi network? Settings and calibration will be kept." => ["Forget the saved Wi-Fi network? Settings and calibration will be kept.", "¿Olvidar la red Wi-Fi? Se conservarán los ajustes y la calibración.", "Gespeichertes WLAN vergessen? Einstellungen und Kalibrierung bleiben erhalten.", "Oublier le Wi-Fi enregistré ? Les réglages et la calibration seront conservés.", "Забути збережену мережу Wi-Fi? Налаштування та калібрування залишаться.", "Glöm det sparade Wi-Fi-nätverket? Inställningar och kalibrering behålls.", "Dimenticare il Wi-Fi salvato? Impostazioni e calibrazione saranno conservate.", "Удалить сохранённую сеть Wi-Fi? Настройки и калибровка сохранятся."][language.index()],
        "Reset" => ["Reset", "Restablecer", "Zurücksetzen", "Réinitialiser", "Скинути", "Återställ", "Reimposta", "Сбросить"][language.index()],
        "Wi-Fi reset failed. Please retry." => ["Wi-Fi reset failed. Please retry.", "No se restableció el Wi-Fi. Reintenta.", "WLAN konnte nicht zurückgesetzt werden.", "Échec de réinitialisation Wi-Fi.", "Не вдалося скинути Wi-Fi. Повторіть.", "Wi-Fi kunde inte återställas.", "Ripristino Wi-Fi fallito. Riprova.", "Не удалось сбросить Wi-Fi. Повторите."][language.index()],
        "Resetting Wi-Fi..." => ["Resetting Wi-Fi...", "Restableciendo Wi-Fi...", "WLAN wird zurückgesetzt...", "Réinitialisation Wi-Fi...", "Скидання Wi-Fi...", "Återställer Wi-Fi...", "Ripristino Wi-Fi...", "Сброс Wi-Fi..."][language.index()],
        "Tap four corner targets, then the center. Lift the stylus between targets. Press BOOT to cancel." => ["Tap four corner targets, then the center. Lift the stylus between targets. Press BOOT to cancel.", "Toca las cuatro esquinas y el centro. Levanta el lápiz entre puntos. BOOT cancela.", "Vier Eckpunkte und die Mitte antippen. Stift dazwischen anheben. BOOT bricht ab.", "Touchez les quatre coins puis le centre. Levez le stylet entre les cibles. BOOT annule.", "Торкніться чотирьох кутових міток, потім центру. Піднімайте стилус між мітками. BOOT скасовує.", "Tryck på fyra hörn och mitten. Lyft pennan mellan målen. BOOT avbryter.", "Tocca quattro angoli e il centro. Solleva la penna tra i punti. BOOT annulla.", "Нажмите четыре угловые метки, затем центр. Убирайте стилус между нажатиями. BOOT отменяет."][language.index()],
        "Touch calibration" => ["Touch calibration", "Calibración táctil", "Touch-Kalibrierung", "Calibration tactile", "Калібрування дотику", "Pekkalibrering", "Calibrazione touch", "Калибровка сенсора"][language.index()],
        "Tap the yellow cross. Lift the stylus before the next target. BOOT cancels." => ["Tap the yellow cross. Lift the stylus before the next target. BOOT cancels.", "Toca la cruz amarilla. Levanta el lápiz antes del siguiente punto. BOOT cancela.", "Gelbes Kreuz antippen. Stift vor dem nächsten Ziel anheben. BOOT bricht ab.", "Touchez la croix jaune. Levez le stylet avant la cible suivante. BOOT annule.", "Торкніться жовтого хрестика. Підніміть стилус перед наступною міткою. BOOT скасовує.", "Tryck på det gula korset. Lyft pennan före nästa mål. BOOT avbryter.", "Tocca la croce gialla. Solleva la penna prima del prossimo punto. BOOT annulla.", "Нажмите жёлтый крест. Уберите стилус перед следующей меткой. BOOT отменяет."][language.index()],
        "Verify calibration" => ["Verify calibration", "Verificar calibración", "Kalibrierung prüfen", "Vérifier la calibration", "Перевірка калібрування", "Kontrollera kalibrering", "Verifica calibrazione", "Проверка калибровки"][language.index()],
        "Tap the yellow cross at the center. BOOT cancels." => ["Tap the yellow cross at the center. BOOT cancels.", "Toca la cruz amarilla del centro. BOOT cancela.", "Gelbes Kreuz in der Mitte antippen. BOOT bricht ab.", "Touchez la croix jaune au centre. BOOT annule.", "Торкніться жовтого хрестика в центрі. BOOT скасовує.", "Tryck på det gula korset i mitten. BOOT avbryter.", "Tocca la croce gialla al centro. BOOT annulla.", "Нажмите жёлтый крест в центре. BOOT отменяет."][language.index()],
        "Calibration did not align. Please retry." => ["Calibration did not align. Please retry.", "La calibración no coincide. Reintenta.", "Kalibrierung stimmt nicht. Erneut versuchen.", "Calibration incorrecte. Réessayez.", "Неточне калібрування. Повторіть.", "Kalibreringen stämde inte. Försök igen.", "Calibrazione errata. Riprova.", "Калибровка неточная. Повторите."][language.index()],
        "Calibration timed out." => ["Calibration timed out.", "Se agotó el tiempo de calibración.", "Zeit für Kalibrierung abgelaufen.", "Délai de calibration dépassé.", "Час калібрування вичерпано.", "Kalibreringen tog för lång tid.", "Tempo di calibrazione scaduto.", "Время калибровки истекло."][language.index()],
        "Calibration cancelled." => ["Calibration cancelled.", "Calibración cancelada.", "Kalibrierung abgebrochen.", "Calibration annulée.", "Калібрування скасовано.", "Kalibreringen avbröts.", "Calibrazione annullata.", "Калибровка отменена."][language.index()],
        "Calibration saved." => ["Calibration saved.", "Calibración guardada.", "Kalibrierung gespeichert.", "Calibration enregistrée.", "Калібрування збережено.", "Kalibreringen sparades.", "Calibrazione salvata.", "Калибровка сохранена."][language.index()],
        "Calibration could not be saved." => ["Calibration could not be saved.", "No se guardó la calibración.", "Kalibrierung konnte nicht gespeichert werden.", "Calibration non enregistrée.", "Не вдалося зберегти калібрування.", "Kalibreringen kunde inte sparas.", "Impossibile salvare la calibrazione.", "Не удалось сохранить калибровку."][language.index()],
        "Wi-Fi setup" => ["Wi-Fi setup", "Configurar Wi-Fi", "WLAN-Einrichtung", "Configuration Wi-Fi", "Налаштування Wi-Fi", "Wi-Fi-inställning", "Configura Wi-Fi", "Настройка Wi-Fi"][language.index()],
        "Choose Wi-Fi" => ["Choose Wi-Fi", "Elegir Wi-Fi", "WLAN auswählen", "Choisir Wi-Fi", "Виберіть Wi-Fi", "Välj Wi-Fi", "Scegli Wi-Fi", "Выберите Wi-Fi"][language.index()],
        "Wi-Fi password" => ["Wi-Fi password", "Contraseña Wi-Fi", "WLAN-Passwort", "Mot de passe Wi-Fi", "Пароль Wi-Fi", "Wi-Fi-lösenord", "Password Wi-Fi", "Пароль Wi-Fi"][language.index()],
        "Connected" => ["Connected", "Conectado", "Verbunden", "Connecté", "Підключено", "Ansluten", "Connesso", "Подключено"][language.index()],
        "Setup required" => ["Setup required", "Configuración necesaria", "Einrichtung erforderlich", "Configuration requise", "Потрібне налаштування", "Inställning krävs", "Configurazione necessaria", "Нужна настройка"][language.index()],
        "Network" => ["Network", "Red", "Netzwerk", "Réseau", "Мережа", "Nätverk", "Rete", "Сеть"][language.index()],
        "Set up new Wi-Fi" => ["Set up new Wi-Fi", "Configurar nuevo Wi-Fi", "Neues WLAN einrichten", "Configurer un nouveau Wi-Fi", "Налаштувати Wi-Fi", "Ställ in nytt Wi-Fi", "Configura nuovo Wi-Fi", "Настроить Wi-Fi"][language.index()],
        "Scan again" => ["Scan again", "Buscar de nuevo", "Erneut suchen", "Rechercher à nouveau", "Сканувати знову", "Sök igen", "Cerca di nuovo", "Повторить поиск"][language.index()],
        "Change Wi-Fi" => ["Change Wi-Fi", "Cambiar Wi-Fi", "WLAN ändern", "Changer Wi-Fi", "Змінити Wi-Fi", "Ändra Wi-Fi", "Cambia Wi-Fi", "Изменить Wi-Fi"][language.index()],
        "Previous" => ["Previous", "Anterior", "Vorherige", "Précédent", "Попередня", "Föregående", "Precedente", "Назад"][language.index()],
        "Next" => ["Next", "Siguiente", "Weiter", "Suivant", "Наступна", "Nästa", "Successivo", "Далее"][language.index()],
        "Refresh" => ["Refresh", "Actualizar", "Aktualisieren", "Actualiser", "Оновити", "Uppdatera", "Aggiorna", "Обновить"][language.index()],
        "Connect" => ["Connect", "Conectar", "Verbinden", "Connecter", "Підключити", "Anslut", "Connetti", "Подключить"][language.index()],
        "Show password" => ["Show password", "Mostrar contraseña", "Passwort anzeigen", "Afficher le mot de passe", "Показати пароль", "Visa lösenord", "Mostra password", "Показать пароль"][language.index()],
        "Hide password" => ["Hide password", "Ocultar contraseña", "Passwort verbergen", "Masquer le mot de passe", "Приховати пароль", "Dölj lösenord", "Nascondi password", "Скрыть пароль"][language.index()],
        "Shift" => ["Shift", "Mayús", "Umschalt", "Maj", "Регістр", "Skift", "Maiusc", "Регистр"][language.index()],
        "Space" => ["Space", "Espacio", "Leer", "Espace", "Пробіл", "Mellanslag", "Spazio", "Пробел"][language.index()],
        "Del" => ["Del", "Borrar", "Lösch", "Suppr", "Видал.", "Radera", "Elimina", "Удалить"][language.index()],
        "Accents" => ["Accents", "Acentos", "Akzente", "Accents", "Акценти", "Accenter", "Accenti", "Акценты"][language.index()],
        "Starting Wi-Fi..." => ["Starting Wi-Fi...", "Iniciando Wi-Fi...", "WLAN startet...", "Démarrage Wi-Fi...", "Запуск Wi-Fi...", "Startar Wi-Fi...", "Avvio Wi-Fi...", "Запуск Wi-Fi..."][language.index()],
        "No Wi-Fi network is configured." => ["No Wi-Fi network is configured.", "No hay red Wi-Fi configurada.", "Kein WLAN eingerichtet.", "Aucun Wi-Fi configuré.", "Мережу Wi-Fi не налаштовано.", "Inget Wi-Fi-nätverk är inställt.", "Nessuna rete Wi-Fi configurata.", "Сеть Wi-Fi не настроена."][language.index()],
        "The saved network is unavailable." => ["The saved network is unavailable.", "La red guardada no está disponible.", "Das gespeicherte WLAN ist nicht verfügbar.", "Le réseau enregistré est indisponible.", "Збережена мережа недоступна.", "Det sparade nätverket är inte tillgängligt.", "La rete salvata non è disponibile.", "Сохранённая сеть недоступна."][language.index()],
        "No Wi-Fi networks found." => ["No Wi-Fi networks found.", "No se encontraron redes Wi-Fi.", "Keine WLAN-Netzwerke gefunden.", "Aucun réseau Wi-Fi trouvé.", "Мереж Wi-Fi не знайдено.", "Inga Wi-Fi-nätverk hittades.", "Nessuna rete Wi-Fi trovata.", "Сети Wi-Fi не найдены."][language.index()],
        "Authentication failed. Check the password." => ["Authentication failed. Check the password.", "Falló la autenticación. Revisa la contraseña.", "Anmeldung fehlgeschlagen. Passwort prüfen.", "Échec d’authentification. Vérifiez le mot de passe.", "Помилка входу. Перевірте пароль.", "Autentisering misslyckades. Kontrollera lösenordet.", "Autenticazione fallita. Controlla la password.", "Ошибка авторизации. Проверьте пароль."][language.index()],
        "Could not obtain an IP address." => ["Could not obtain an IP address.", "No se obtuvo una dirección IP.", "Keine IP-Adresse erhalten.", "Impossible d’obtenir une adresse IP.", "Не вдалося отримати IP-адресу.", "Kunde inte få en IP-adress.", "Impossibile ottenere un indirizzo IP.", "Не удалось получить IP-адрес."][language.index()],
        "Wi-Fi scan failed. Please retry." => ["Wi-Fi scan failed. Please retry.", "Falló la búsqueda Wi-Fi. Reintenta.", "WLAN-Suche fehlgeschlagen. Erneut versuchen.", "Échec de recherche Wi-Fi. Réessayez.", "Помилка сканування Wi-Fi. Повторіть.", "Wi-Fi-sökning misslyckades. Försök igen.", "Ricerca Wi-Fi fallita. Riprova.", "Ошибка поиска Wi-Fi. Повторите."][language.index()],
        "Could not connect. Please retry." => ["Could not connect. Please retry.", "No se pudo conectar. Reintenta.", "Verbindung fehlgeschlagen. Erneut versuchen.", "Connexion impossible. Réessayez.", "Не вдалося підключитися. Повторіть.", "Kunde inte ansluta. Försök igen.", "Impossibile connettersi. Riprova.", "Не удалось подключиться. Повторите."][language.index()],
        "Could not save Wi-Fi. Please try again." => ["Could not save Wi-Fi. Please try again.", "No se guardó el Wi-Fi. Reintenta.", "WLAN konnte nicht gespeichert werden.", "Enregistrement Wi-Fi impossible.", "Не вдалося зберегти Wi-Fi. Повторіть.", "Kunde inte spara Wi-Fi. Försök igen.", "Impossibile salvare Wi-Fi. Riprova.", "Не удалось сохранить Wi-Fi. Повторите."][language.index()],
        "Select a secured 2.4 GHz network." => ["Select a secured 2.4 GHz network.", "Selecciona una red segura de 2,4 GHz.", "Gesichertes 2,4-GHz-WLAN auswählen.", "Choisissez un réseau sécurisé de 2,4 GHz.", "Виберіть захищену мережу 2,4 ГГц.", "Välj ett säkert 2,4 GHz-nätverk.", "Scegli una rete protetta a 2,4 GHz.", "Выберите защищённую сеть 2,4 ГГц."][language.index()],
        "No Wi-Fi is configured. No Wi-Fi networks found." => ["No Wi-Fi is configured. No Wi-Fi networks found.", "No hay Wi-Fi configurado ni redes disponibles.", "Kein WLAN eingerichtet und keine Netzwerke gefunden.", "Aucun Wi-Fi configuré ni réseau trouvé.", "Wi-Fi не налаштовано. Мереж не знайдено.", "Inget Wi-Fi inställt och inga nätverk hittades.", "Wi-Fi non configurato. Nessuna rete trovata.", "Wi-Fi не настроен. Сети не найдены."][language.index()],
        "Wi-Fi connected." => ["Wi-Fi connected.", "Wi-Fi conectado.", "WLAN verbunden.", "Wi-Fi connecté.", "Wi-Fi підключено.", "Wi-Fi anslutet.", "Wi-Fi connesso.", "Wi-Fi подключён."][language.index()],
        "Scanning..." => ["Scanning...", "Buscando redes...", "Netzwerke werden gesucht...", "Recherche de réseaux...", "Сканування...", "Söker nätverk...", "Ricerca reti...", "Поиск сетей..."][language.index()],
        "Reconnecting..." => ["Reconnecting...", "Reconectando...", "Verbindung wird hergestellt...", "Reconnexion...", "Повторне підключення...", "Återansluter...", "Riconnessione...", "Повторное подключение..."][language.index()],
        "Connecting..." => ["Connecting...", "Conectando...", "Verbindung wird hergestellt...", "Connexion...", "Підключення...", "Ansluter...", "Connessione...", "Подключение..."][language.index()],
        "Enter the Wi-Fi password." => ["Enter the Wi-Fi password.", "Introduce la contraseña Wi-Fi.", "WLAN-Passwort eingeben.", "Saisissez le mot de passe Wi-Fi.", "Введіть пароль Wi-Fi.", "Ange Wi-Fi-lösenordet.", "Inserisci la password Wi-Fi.", "Введите пароль Wi-Fi."][language.index()],
        "Wi-Fi setup cancelled." => ["Wi-Fi setup cancelled.", "Configuración Wi-Fi cancelada.", "WLAN-Einrichtung abgebrochen.", "Configuration Wi-Fi annulée.", "Налаштування Wi-Fi скасовано.", "Wi-Fi-inställning avbruten.", "Configurazione Wi-Fi annullata.", "Настройка Wi-Fi отменена."][language.index()],
        "Open, WEP and enterprise Wi-Fi are unsupported." => ["Open, WEP and enterprise Wi-Fi are unsupported.", "Wi-Fi abierto, WEP y empresarial no compatibles.", "Offene, WEP- und Unternehmensnetze werden nicht unterstützt.", "Wi-Fi ouvert, WEP et entreprise non pris en charge.", "Відкриті мережі, WEP та корпоративні Wi-Fi не підтримуються.", "Öppna nätverk, WEP och företags-Wi-Fi stöds inte.", "Wi-Fi aperto, WEP e aziendale non supportati.", "Открытые сети, WEP и корпоративный Wi-Fi не поддерживаются."][language.index()],
        "Use a password of 8-63 characters." => ["Use a password of 8-63 characters.", "Usa una contraseña de 8 a 63 caracteres.", "Passwort mit 8 bis 63 Zeichen verwenden.", "Utilisez un mot de passe de 8 à 63 caractères.", "Пароль має містити 8–63 символи.", "Använd ett lösenord med 8–63 tecken.", "Usa una password di 8–63 caratteri.", "Пароль должен содержать 8–63 символа."][language.index()],
        "Invalid network name." => ["Invalid network name.", "Nombre de red inválido.", "Ungültiger Netzwerkname.", "Nom de réseau incorrect.", "Неприпустима назва мережі.", "Ogiltigt nätverksnamn.", "Nome rete non valido.", "Недопустимое имя сети."][language.index()],
        "This network security is unsupported." => ["This network security is unsupported.", "Seguridad de red no compatible.", "Netzwerksicherheit wird nicht unterstützt.", "Sécurité réseau non prise en charge.", "Цей захист мережі не підтримується.", "Nätverkssäkerheten stöds inte.", "Sicurezza rete non supportata.", "Защита этой сети не поддерживается."][language.index()],
        "Device error. Please restart." => ["Device error. Please restart.", "Error del dispositivo. Reinicia.", "Gerätefehler. Bitte neu starten.", "Erreur de l’appareil. Redémarrez.", "Помилка пристрою. Перезапустіть.", "Enhetsfel. Starta om.", "Errore del dispositivo. Riavvia.", "Ошибка устройства. Перезапустите."][language.index()],
        "Starting weather display..." => ["Starting weather display...", "Iniciando pantalla...", "Wetteranzeige startet...", "Démarrage de l’écran...", "Запуск погодного екрана...", "Startar väderskärmen...", "Avvio schermo meteo...", "Запуск погодного экрана..."][language.index()],
        "Hold BOOT now to calibrate touch." => ["Hold BOOT now to calibrate touch.", "Mantén BOOT para calibrar.", "BOOT jetzt zur Kalibrierung halten.", "Maintenez BOOT pour calibrer.", "Утримуйте BOOT для калібрування.", "Håll BOOT för kalibrering.", "Tieni BOOT per calibrare.", "Удерживайте BOOT для калибровки сенсора."][language.index()],
        "Tap the yellow cross with a stylus." => ["Tap the yellow cross with a stylus.", "Toca la cruz amarilla con el lápiz.", "Gelbes Kreuz mit dem Stift antippen.", "Touchez la croix jaune avec le stylet.", "Торкніться жовтого хрестика стилусом.", "Tryck på det gula korset med pennan.", "Tocca la croce gialla con la penna.", "Нажмите жёлтый крест стилусом."][language.index()],
        "Lift the stylus between targets." => ["Lift the stylus between targets.", "Levanta el lápiz entre los puntos.", "Stift zwischen den Zielen anheben.", "Levez le stylet entre les cibles.", "Піднімайте стилус між мітками.", "Lyft pennan mellan målen.", "Solleva la penna tra i punti.", "Убирайте стилус между нажатиями."][language.index()],
        "Tap the yellow cross at the center." => ["Tap the yellow cross at the center.", "Toca la cruz amarilla del centro.", "Gelbes Kreuz in der Mitte antippen.", "Touchez la croix jaune au centre.", "Торкніться жовтого хрестика в центрі.", "Tryck på det gula korset i mitten.", "Tocca la croce gialla al centro.", "Нажмите жёлтый крест в центре."][language.index()],
        "Calibration did not align." => ["Calibration did not align.", "La calibración no coincide.", "Kalibrierung stimmt nicht.", "Calibration incorrecte.", "Неточне калібрування.", "Kalibreringen stämde inte.", "Calibrazione errata.", "Калибровка неточная."][language.index()],
        "Please try the targets again." => ["Please try the targets again.", "Prueba los puntos de nuevo.", "Ziele bitte erneut antippen.", "Réessayez les cibles.", "Спробуйте торкнутися міток знову.", "Försök med målen igen.", "Riprova i punti.", "Нажмите метки ещё раз."][language.index()],
        "Touch storage could not be read." => ["Touch storage could not be read.", "No se leyó la calibración.", "Touch-Speicher konnte nicht gelesen werden.", "Lecture de calibration impossible.", "Не вдалося прочитати калібрування.", "Peklagringen kunde inte läsas.", "Impossibile leggere la calibrazione.", "Не удалось прочитать калибровку."][language.index()],
        "Restart and check device storage." => ["Restart and check device storage.", "Reinicia y revisa el almacenamiento.", "Neu starten und Gerätespeicher prüfen.", "Redémarrez et vérifiez le stockage.", "Перезапустіть і перевірте пам’ять.", "Starta om och kontrollera lagringen.", "Riavvia e controlla la memoria.", "Перезапустите и проверьте память устройства."][language.index()],
        "Touch calibration could not be saved." => ["Touch calibration could not be saved.", "No se guardó la calibración táctil.", "Touch-Kalibrierung konnte nicht gespeichert werden.", "Calibration tactile non enregistrée.", "Не вдалося зберегти калібрування дотику.", "Pekkalibreringen kunde inte sparas.", "Calibrazione touch non salvata.", "Не удалось сохранить калибровку сенсора."][language.index()],
        other => other,
    }
}
/// Translates each line of a prompt independently.
///
/// # Arguments
///
/// * `language` (`Language`) - Supported language used for translated labels or API
///   requests.
/// * `text` (`&str`) - Text to translate, measure, clip, or draw.
///
/// # Returns
///
/// `String` - Owned translated lines joined by newline characters.
pub fn multiline(language: Language, text: &str) -> String {
    // Execute join for <Vec< >> result for the surrounding operation.
    text.split('\n')
        .map(|line| tr(language, line))
        .collect::<Vec<_>>()
        .join("\n")
}

///////////////////////////////////////////////////////////////////////////////////////
//                                    UNIT TESTS                                     //
///////////////////////////////////////////////////////////////////////////////////////

/// Unit tests and supporting fixtures for i18n behavior.
#[cfg(test)]
mod tests {
    use super::*;
    /// Verifies stable persisted language indices and the expected supported-language mappings.
    ///
    /// # Arguments
    ///
    /// None.
    ///
    /// # Returns
    ///
    /// `()` - No value; completes when the expected test assertions hold.
    ///
    /// # Panics
    ///
    /// Assertions or fixture assumptions panic if the tested behavior is violated.
    #[test]
    fn language_indices_preserve_existing_slots() {
        // Visit each entry in enumerate result for the surrounding operation; the loop binding
        // provides its value or index for this iteration.
        for (index, language) in [
            Language::English,
            Language::Spanish,
            Language::German,
            Language::French,
            Language::Ukrainian,
            Language::Swedish,
            Language::Italian,
        ]
        .into_iter()
        .enumerate()
        {
            // Verify that returns the language's stable persisted position exactly matches
            // zero-based position of the selected entry.
            assert_eq!(language.index(), index);
            // Verify that looks up a supported language by its persisted index exactly matches an
            // available value for selected language for interface text and location search.
            assert_eq!(Language::from_index(index as u32), Some(language));
        }
        // Verify that looks up a supported language by its persisted index exactly matches an
        // available value for Language Russian.
        assert_eq!(Language::from_index(7), Some(Language::Russian));
        // Verify that looks up a supported language by its persisted index exactly matches no
        // available value.
        assert_eq!(Language::from_index(8), None);
        // Verify that returns the language's native display name exactly matches the specified
        // message, format, or data literal.
        assert_eq!(Language::Ukrainian.name(), "Українська");
        // Verify that returns the language code used in location requests exactly matches the
        // specified message, format, or data literal.
        assert_eq!(Language::Ukrainian.code(), "uk");
        // Verify that translates a known interface string into the selected language exactly
        // matches the specified message, format, or data literal.
        assert_eq!(tr(Language::Ukrainian, "Settings"), "Налаштування");
        // Verify that returns the language's native display name exactly matches the specified
        // message, format, or data literal.
        assert_eq!(Language::Russian.name(), "Русский");
        // Verify that returns the language code used in location requests exactly matches the
        // specified message, format, or data literal.
        assert_eq!(Language::Russian.code(), "ru");
        // Verify that translates a known interface string into the selected language exactly
        // matches the specified message, format, or data literal.
        assert_eq!(tr(Language::Russian, "Settings"), "Настройки");
        // Verify that translates each line of a prompt independently exactly matches the specified
        // message, format, or data literal.
        assert_eq!(
            multiline(Language::Russian, "Settings\nBack"),
            "Настройки\nНазад"
        );
    }
}
