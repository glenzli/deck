#include "process_liveness.hpp"
#include <QFileInfo>
#ifdef Q_OS_WIN
#include <windows.h>
#elif defined(Q_OS_MACOS)
#include <libproc.h>
#else
#include <QFile>
#endif
bool externalProcessAlive(qint64 processId, const QString& executable) {
    if (processId <= 0)
        return false;
    QString actual;
#ifdef Q_OS_WIN
    const auto handle =
        OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, FALSE, static_cast<DWORD>(processId));
    if (!handle)
        return false;
    wchar_t buffer[32768];
    DWORD size = 32768;
    if (QueryFullProcessImageNameW(handle, 0, buffer, &size))
        actual = QString::fromWCharArray(buffer, static_cast<qsizetype>(size));
    CloseHandle(handle);
#elif defined(Q_OS_MACOS)
    char buffer[PROC_PIDPATHINFO_MAXSIZE];
    if (proc_pidpath(static_cast<int>(processId), buffer, sizeof(buffer)) > 0)
        actual = QString::fromUtf8(buffer);
#else
    actual = QFileInfo(QString("/proc/%1/exe").arg(processId)).symLinkTarget();
#endif
    return !actual.isEmpty()
           && QFileInfo(actual).canonicalFilePath() == QFileInfo(executable).canonicalFilePath();
}
