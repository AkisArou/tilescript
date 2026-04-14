#pragma once

#include <string>

#include "src/SharedDefs.hpp"

namespace hypreact_plugin {

std::string queryRuntime(eHyprCtlOutputFormat format, std::string arg,
                         void (*resyncAll)());

} // namespace hypreact_plugin
