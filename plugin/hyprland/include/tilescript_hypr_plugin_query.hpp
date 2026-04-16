#pragma once

#include <string>

#include "src/SharedDefs.hpp"

namespace tilescript_plugin {

std::string queryRuntime(eHyprCtlOutputFormat format, std::string arg,
                         void (*resyncAll)());

} // namespace tilescript_plugin
