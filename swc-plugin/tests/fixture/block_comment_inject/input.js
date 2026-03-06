// Case 1: block comment on preceding line, function declaration
/* @ngInject */
function case1($scope, $http) {}

// Case 2: block comment on preceding line, var + function expression
/* @ngInject */
var case2 = function($scope) {};

// Case 3: block comment on preceding line, var + arrow
/* @ngInject */
var case3 = ($a, $b) => {};

// Case 4: inline before function expression in var
var case4 = /* @ngInject */ function($scope) {};

// Case 5: inline before function arg in Angular call (already a suspect, but should not double-wrap)
myMod.controller("c5", /* @ngInject */ function($scope) {});

// Case 6: block comment inline on same line as function declaration
/* @ngInject */ function case6($scope) {}
