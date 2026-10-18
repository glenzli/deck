#include "deck_document_controller.hpp"
#include "desktop_backend.hpp"
#include <QClipboard>
#include <QDebug>
#include <QDir>
#include <QFileInfo>
#include <QGuiApplication>
#include <QJsonDocument>
#include <QProcess>
#include <QStandardPaths>
#include <stdexcept>
DeckDocumentController::DeckDocumentController(
    DesktopBackend& backend,
    const DeckCliClient& cli,
    const QString& settingsPath,
    QObject* parent
)
    : QObject(parent), backend_(backend), cli_(cli), settings_(settingsPath, QSettings::IniFormat) {
}
bool DeckDocumentController::fail(const QString& message, const std::exception& error) {
    qWarning().noquote() << "Deck document:" << error.what();
    error_ = message;
    emit statusChanged();
    return false;
}
QStringList DeckDocumentController::recentProjects() const {
    return settings_.value("deck/recent_projects").toStringList();
}
void DeckDocumentController::remember() {
    auto projects = recentProjects();
    projects.removeAll(backend_.bundlePath());
    projects.prepend(backend_.bundlePath());
    while (projects.size() > 12)
        projects.removeLast();
    settings_.setValue("deck/recent_projects", projects);
    emit recentProjectsChanged();
}
bool DeckDocumentController::createProject(const QUrl& parent, const QString& name) {
    const auto label = name.trimmed();
    if (!parent.isLocalFile() || !QFileInfo(parent.toLocalFile()).isDir() || label.isEmpty()
        || label.contains('/') || label.contains('\\') || label == "." || label == "..") {
        error_ = QString::fromUtf8(QT_TR_NOOP("Choose a local folder and enter a project name."));
        emit statusChanged();
        return false;
    }
    const auto path = QDir(parent.toLocalFile()).filePath(label + ".deck");
    if (QFileInfo::exists(path)) {
        error_ = QString::fromUtf8(
            QT_TR_NOOP("A project with this name already exists. Choose another name.")
        );
        emit statusChanged();
        return false;
    }
    try {
        // An empty card project has no audio graphs to initialize. Existing projects use the strict
        // guard below.
        auto created =
            shape::desktop::create_desktop_project(path.toStdString(), label.toStdString());
        Q_UNUSED(created);
        if (!backend_.openProject(QUrl::fromLocalFile(path)))
            throw std::runtime_error("new project could not be opened");
        error_.clear();
        remember();
        emit statusChanged();
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(QT_TR_NOOP(
                "Could not create this Deck project. Choose another folder and try again."
            )),
            error
        );
    }
}
bool DeckDocumentController::openProject(const QUrl& path) {
    try {
        if (!path.isLocalFile())
            throw std::runtime_error("local project required");
        const auto inspected = cli_.run({"status", path.toLocalFile()});
        const auto guard = inspected["open_guard"].toObject();
        if (!guard["allowed"].toBool()) {
            error_ = QString::fromUtf8(QT_TR_NOOP(
                "This project contains other Shape workflows. Open it in Shape and choose "
                "Continue in Deck to copy the selected cards into a Deck project."
            ));
            emit statusChanged();
            return false;
        }
        if (!backend_.openProject(path))
            throw std::runtime_error("project session failed");
        error_.clear();
        remember();
        emit statusChanged();
        return true;
    } catch (const std::exception& error) {
        return fail(
            QString::fromUtf8(
                QT_TR_NOOP("Could not open this Deck project. Check the folder and try again.")
            ),
            error
        );
    }
}
QString DeckDocumentController::shapeExecutable() const {
    const auto configured = settings_.value("deck/shape_executable").toString();
    auto executable =
        configured.isEmpty() ? QStandardPaths::findExecutable("shape-desktop") : configured;
#ifdef Q_OS_MACOS
    if (executable.isEmpty()
        && QFileInfo("/Applications/Shape.app/Contents/MacOS/shape-desktop").isExecutable())
        executable = "/Applications/Shape.app/Contents/MacOS/shape-desktop";
#endif
    const auto resolved = QFileInfo(executable).canonicalFilePath();
    return resolved.isEmpty() ? executable : resolved;
}
bool DeckDocumentController::setShapeExecutable(const QUrl& url) {
    auto path = url.toLocalFile();
#ifdef Q_OS_MACOS
    if (path.endsWith(".app", Qt::CaseInsensitive))
        path += "/Contents/MacOS/shape-desktop";
#endif
    path = QDir::cleanPath(QFileInfo(path).absoluteFilePath());
    if (!url.isLocalFile() || !QFileInfo(path).isExecutable()) {
        error_ = QString::fromUtf8(QT_TR_NOOP("Choose the Shape application or executable."));
        emit statusChanged();
        return false;
    }
    // Preserve the selected update alias; callers capture its current executable when starting.
    settings_.setValue("deck/shape_executable", path);
    error_.clear();
    emit statusChanged();
    return true;
}
bool DeckDocumentController::openShape() {
    const auto path = shapeExecutable();
    if (path.isEmpty() || !QProcess::startDetached(path, {})) {
        error_ = QString::fromUtf8(QT_TR_NOOP("Choose Shape in Settings before opening it."));
        emit statusChanged();
        return false;
    }
    error_.clear();
    emit statusChanged();
    return true;
}
void DeckDocumentController::copyText(const QString& text) {
    QGuiApplication::clipboard()->setText(text);
}
