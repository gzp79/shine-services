cmake_minimum_required(VERSION 3.4.1)
project(GameActivity LANGUAGES CXX)

#find_package(game-activity REQUIRED CONFIG)

set(SRC_DIR ./src/main/cpp)

add_library(gameActivity SHARED ${SRC_DIR}/dummy.cpp)
target_link_libraries(gameActivity PUBLIC 
    log 
    android 
    #game-activity::game-activity_static
)