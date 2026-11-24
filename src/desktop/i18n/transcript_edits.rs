//! Labs › Delete transcript lines and Play meeting audio: one-language
//! strings for their lab cards, the selection bar, the player, and their
//! failure notices, kept beside the main language tables.

/// French strings by their English source.
#[rustfmt::skip]
pub(super) const FRENCH: &[(&str, &str)] = &[
    ("Play meeting audio", "Lire l’audio des réunions"),
    ("Listen to a saved meeting while its transcript follows along. Click a line to hear it, or export the audio beside the Markdown", "Écoutez une réunion enregistrée pendant que la transcription suit la lecture. Cliquez sur une ligne pour l’entendre, ou exportez l’audio à côté du fichier Markdown"),
    ("Play from here", "Lire à partir d’ici"),
    ("Back 15 seconds", "Reculer de 15 secondes"),
    ("Forward 15 seconds", "Avancer de 15 secondes"),
    ("Audio position", "Position de l’audio"),
    ("Export audio", "Exporter l’audio"),
    ("Exporting audio…", "Exportation de l’audio…"),
    ("Audio could not be exported.", "Impossible d’exporter l’audio."),
    ("Delete transcript lines", "Supprimer des lignes de transcription"),
    ("Select lines in a saved meeting transcript and delete them. Summaries, copies, and the Markdown file use what's left", "Sélectionnez des lignes dans la transcription d’une réunion enregistrée et supprimez-les. Les résumés, les copies et le fichier Markdown utilisent ce qui reste"),
    ("Select lines", "Sélectionner des lignes"),
    ("{} lines selected", "{} lignes sélectionnées"),
    ("1 line selected", "1 ligne sélectionnée"),
    ("Delete lines…", "Supprimer les lignes…"),
    ("Delete {} lines", "Supprimer {} lignes"),
    ("Delete 1 line", "Supprimer 1 ligne"),
    ("Select the lines to delete.", "Sélectionnez les lignes à supprimer."),
    ("Its summary and chapters will be removed. Summarize again for a fresh one.", "Son résumé et ses chapitres seront supprimés. Relancez le résumé pour en créer de nouveaux."),
    ("Lines could not be deleted.", "Impossible de supprimer les lignes."),
];

/// Portuguese strings by their English source.
#[rustfmt::skip]
pub(super) const PORTUGUESE: &[(&str, &str)] = &[
    ("Play meeting audio", "Reproduzir o áudio das reuniões"),
    ("Listen to a saved meeting while its transcript follows along. Click a line to hear it, or export the audio beside the Markdown", "Ouça uma reunião salva enquanto a transcrição acompanha a reprodução. Clique em uma linha para ouvi-la ou exporte o áudio ao lado do Markdown"),
    ("Play from here", "Reproduzir daqui"),
    ("Back 15 seconds", "Voltar 15 segundos"),
    ("Forward 15 seconds", "Avançar 15 segundos"),
    ("Audio position", "Posição do áudio"),
    ("Export audio", "Exportar áudio"),
    ("Exporting audio…", "Exportando áudio…"),
    ("Audio could not be exported.", "Não foi possível exportar o áudio."),
    ("Delete transcript lines", "Excluir linhas da transcrição"),
    ("Select lines in a saved meeting transcript and delete them. Summaries, copies, and the Markdown file use what's left", "Selecione linhas na transcrição de uma reunião salva e exclua-as. Resumos, cópias e o arquivo Markdown usam o que restar"),
    ("Select lines", "Selecionar linhas"),
    ("{} lines selected", "{} linhas selecionadas"),
    ("1 line selected", "1 linha selecionada"),
    ("Delete lines…", "Excluir linhas…"),
    ("Delete {} lines", "Excluir {} linhas"),
    ("Delete 1 line", "Excluir 1 linha"),
    ("Select the lines to delete.", "Selecione as linhas que deseja excluir."),
    ("Its summary and chapters will be removed. Summarize again for a fresh one.", "O resumo e os capítulos serão removidos. Resuma novamente para criar novos."),
    ("Lines could not be deleted.", "Não foi possível excluir as linhas."),
];

/// Russian strings by their English source.
#[rustfmt::skip]
pub(super) const RUSSIAN: &[(&str, &str)] = &[
    ("Play meeting audio", "Воспроизведение аудио встреч"),
    ("Listen to a saved meeting while its transcript follows along. Click a line to hear it, or export the audio beside the Markdown", "Слушайте сохранённую встречу, а расшифровка будет следовать за записью. Нажмите на строку, чтобы услышать её, или экспортируйте аудио рядом с файлом Markdown"),
    ("Play from here", "Воспроизвести отсюда"),
    ("Back 15 seconds", "Назад на 15 секунд"),
    ("Forward 15 seconds", "Вперёд на 15 секунд"),
    ("Audio position", "Позиция аудио"),
    ("Export audio", "Экспортировать аудио"),
    ("Exporting audio…", "Экспорт аудио…"),
    ("Audio could not be exported.", "Не удалось экспортировать аудио."),
    ("Delete transcript lines", "Удаление строк расшифровки"),
    ("Select lines in a saved meeting transcript and delete them. Summaries, copies, and the Markdown file use what's left", "Выбирайте строки в сохранённой расшифровке встречи и удаляйте их. Сводки, копии и файл Markdown используют оставшееся"),
    ("Select lines", "Выбрать строки"),
    ("{} lines selected", "Выбрано строк: {}"),
    ("1 line selected", "Выбрана 1 строка"),
    ("Delete lines…", "Удалить строки…"),
    ("Delete {} lines", "Удалить строк: {}"),
    ("Delete 1 line", "Удалить 1 строку"),
    ("Select the lines to delete.", "Выберите строки для удаления."),
    ("Its summary and chapters will be removed. Summarize again for a fresh one.", "Сводка и разделы будут удалены. Запустите сводку снова, чтобы создать новые."),
    ("Lines could not be deleted.", "Не удалось удалить строки."),
];

/// Hungarian, for the strings above.
#[rustfmt::skip]
pub(super) const HUNGARIAN: &[(&str, &str)] = &[
    ("Delete transcript lines", "Átirat sorainak törlése"),
    ("Select lines in a saved meeting transcript and delete them. Summaries, copies, and the Markdown file use what's left", "Jelöljön ki sorokat egy mentett megbeszélés átiratában, és törölje őket. Az összefoglalók, a másolatok és a Markdown-fájl a maradékot használják"),
    ("Select lines", "Sorok kijelölése"),
    ("{} lines selected", "{} sor kijelölve"),
    ("1 line selected", "1 sor kijelölve"),
    ("Delete lines…", "Sorok törlése…"),
    ("Delete {} lines", "{} sor törlése"),
    ("Delete 1 line", "1 sor törlése"),
    ("Select the lines to delete.", "Jelölje ki a törölni kívánt sorokat."),
    ("Its summary and chapters will be removed. Summarize again for a fresh one.", "Az összefoglalója és a fejezetei törlődnek. Új összefoglalóhoz futtassa újra az összegzést."),
    ("Lines could not be deleted.", "A sorokat nem sikerült törölni."),
];
