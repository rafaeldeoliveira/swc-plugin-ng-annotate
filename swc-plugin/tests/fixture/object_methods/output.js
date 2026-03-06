// Object method annotation via @ngInject
var objectTest = {
    /** @ngInject */ foo: [
        "$q",
        function($q) {}
    ]
};
