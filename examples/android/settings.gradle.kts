// SPDX-License-Identifier: GPL-3.0-or-later

pluginManagement {
    repositories {
        google()
        mavenCentral()
        gradlePluginPortal()
    }
}

dependencyResolutionManagement {
    repositoriesMode.set(RepositoriesMode.FAIL_ON_PROJECT_REPOS)
    repositories {
        google()
        mavenCentral()
    }
}

rootProject.name = "walkit-example"
include(":app")

// The library from this repository's source: Gradle builds the AAR, native engine included, as
// part of this build. An app that takes the published AAR leaves this block out.
includeBuild("../../android") {
    dependencySubstitution {
        substitute(module("nl.mattix.walkit:walkit")).using(project(":walkit"))
    }
}
