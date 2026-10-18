//! Read-only external-process identity checks for resuming a local handoff.
#pragma once
#include <QString>
bool externalProcessAlive(qint64 processId, const QString& executable);
