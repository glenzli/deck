//! File-only PDF export using the same physical painter as the desktop.
#pragma once
#include <QString>
class DeckCliClient;
int runPdfCommand(const DeckCliClient&, const QString& requestPath, const QString& resultPath);
